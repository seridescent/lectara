use anyhow::Result;
use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

mod common;

mod helpers {
    use super::*;
    use crate::common::establish_test_connection;
    use lectara_service::{DefaultAppState, routes};

    pub fn create_test_server() -> (TestServer, Arc<Mutex<diesel::sqlite::SqliteConnection>>) {
        let connection = establish_test_connection();
        let db = Arc::new(Mutex::new(connection));

        let state = DefaultAppState::new(db.clone());
        let app = routes::create_router().with_state(state);

        let server = TestServer::new(app).unwrap();
        (server, db)
    }
}

#[tokio::test]
async fn test_add_content_endpoint() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    let content_payload = json!({
        "url": "https://example.com/test-article",
        "title": "Test Article",
        "author": "Test Author"
    });

    let response = server.post("/api/v1/content").json(&content_payload).await;

    response.assert_status_ok();
    let json_response: Value = response.json();
    assert!(json_response["id"].is_number());

    // Verify database state
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();

        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/test-article")
                .expect("Content item should exist in database");

        assert_eq!(saved_item.url, "https://example.com/test-article");
        assert_eq!(saved_item.title, Some("Test Article".to_string()));
        assert_eq!(saved_item.author, Some("Test Author".to_string()));
    }
    Ok(())
}

#[tokio::test]
async fn test_add_content_minimal_payload() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    let content_payload = json!({
        "url": "https://example.com/minimal"
    });

    let response = server.post("/api/v1/content").json(&content_payload).await;

    response.assert_status_ok();

    // Verify database state
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();

        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/minimal")
                .expect("Content item should exist in database");

        assert_eq!(saved_item.url, "https://example.com/minimal");
        assert!(saved_item.title.is_none());
        assert!(saved_item.author.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn test_multiple_content_items() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    // Add first item
    let first_payload = json!({
        "url": "https://example.com/first",
        "title": "First Article"
    });

    server.post("/api/v1/content").json(&first_payload).await;

    // Add second item
    let second_payload = json!({
        "url": "https://example.com/second",
        "author": "Second Author"
    });

    server.post("/api/v1/content").json(&second_payload).await;

    // Verify database state
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();

        assert_eq!(test_utils::count_content_items(&mut conn), 2);

        let all_items = test_utils::get_all_content_items(&mut conn);
        assert_eq!(all_items.len(), 2);

        let urls: Vec<String> = all_items.iter().map(|item| item.url.clone()).collect();
        assert!(urls.contains(&"https://example.com/first".to_string()));
        assert!(urls.contains(&"https://example.com/second".to_string()));
    }
    Ok(())
}

#[tokio::test]
async fn test_duplicate_url_handling() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    // Add first item
    let first_payload = json!({
        "url": "https://example.com/duplicate-test",
        "title": "First Title",
        "author": "First Author"
    });

    let response1 = server.post("/api/v1/content").json(&first_payload).await;
    response1.assert_status_ok();
    let json_response1: Value = response1.json();
    let _first_id = json_response1["id"].as_u64().unwrap();

    // Attempt to add same URL again with different metadata
    let second_payload = json!({
        "url": "https://example.com/duplicate-test",
        "title": "Second Title",
        "author": "Second Author"
    });

    let response2 = server.post("/api/v1/content").json(&second_payload).await;

    // Should return conflict error for different metadata
    response2.assert_status(StatusCode::CONFLICT);

    // Verify only one record exists
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/duplicate-test")
                .expect("Content item should exist in database");

        // Should keep original metadata
        assert_eq!(saved_item.title, Some("First Title".to_string()));
        assert_eq!(saved_item.author, Some("First Author".to_string()));
    }
    Ok(())
}

#[tokio::test]
async fn test_true_idempotent_behavior() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    // Add first item
    let payload = json!({
        "url": "https://example.com/idempotent-test",
        "title": "Same Title",
        "author": "Same Author"
    });

    let response1 = server.post("/api/v1/content").json(&payload).await;
    response1.assert_status_ok();
    let json_response1: Value = response1.json();
    let first_id = json_response1["id"].as_u64().unwrap();

    // Add same item again with identical metadata - should be idempotent
    let response2 = server.post("/api/v1/content").json(&payload).await;

    // Should return existing record (truly idempotent)
    response2.assert_status_ok();
    let json_response2: Value = response2.json();
    assert_eq!(json_response2["id"].as_u64().unwrap(), first_id);

    // Verify only one record exists
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);
    }
    Ok(())
}

#[tokio::test]
async fn test_url_normalization() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    // Add URL with fragment and trailing slash
    let payload1 = json!({
        "url": "https://example.com/article/#section1",
        "title": "Test Article"
    });

    let response1 = server.post("/api/v1/content").json(&payload1).await;
    response1.assert_status_ok();
    let json_response1: Value = response1.json();
    let first_id = json_response1["id"].as_u64().unwrap();

    // Try same URL without fragment - should be treated as duplicate with same metadata
    let payload2 = json!({
        "url": "https://example.com/article/",
        "title": "Test Article"
    });

    let response2 = server.post("/api/v1/content").json(&payload2).await;
    response2.assert_status_ok();
    let json_response2: Value = response2.json();
    assert_eq!(json_response2["id"].as_u64().unwrap(), first_id);

    // Verify only one record and URL is normalized
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let all_items = test_utils::get_all_content_items(&mut conn);
        let saved_url = &all_items[0].url;

        // URL should be normalized (no fragment, no trailing slash)
        assert_eq!(saved_url, "https://example.com/article");
    }
    Ok(())
}

#[tokio::test]
async fn test_invalid_url_rejection() -> Result<()> {
    let (server, _db) = helpers::create_test_server();

    let test_cases = vec![
        ("", "empty URL"),
        ("not-a-url", "malformed URL"),
        ("ftp://example.com", "unsupported protocol"),
        ("https://", "incomplete URL"),
        ("javascript:alert('xss')", "dangerous protocol"),
    ];

    for (invalid_url, description) in test_cases {
        let payload = json!({
            "url": invalid_url,
            "title": format!("Test case: {}", description)
        });

        let response = server.post("/api/v1/content").json(&payload).await;

        // Should reject with 400 Bad Request
        response.assert_status(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

#[tokio::test]
async fn test_invalid_content_type() -> Result<()> {
    let (server, _db) = helpers::create_test_server();

    let response = server
        .post("/api/v1/content")
        .content_type("text/plain") // Wrong content type
        .text(r#"{"url": "https://example.com"}"#)
        .await;

    // Axum should reject non-JSON content types
    response.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    Ok(())
}

#[tokio::test]
async fn test_missing_content_type() -> Result<()> {
    let (server, _db) = helpers::create_test_server();

    let response = server
        .post("/api/v1/content")
        // No content-type header
        .text(r#"{"url": "https://example.com"}"#)
        .await;

    // Axum should reject missing content types for JSON extraction
    response.assert_status(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    Ok(())
}

#[tokio::test]
async fn test_url_with_query_parameters() -> Result<()> {
    let (server, db) = helpers::create_test_server();

    // URLs with different query parameter orders should normalize to same URL
    let payload1 = json!({
        "url": "https://example.com/search?q=rust&sort=date&page=1",
        "title": "Search Results"
    });

    let response1 = server.post("/api/v1/content").json(&payload1).await;
    response1.assert_status_ok();
    let json_response1: Value = response1.json();
    let first_id = json_response1["id"].as_u64().unwrap();

    // Same parameters in different order with same metadata
    let payload2 = json!({
        "url": "https://example.com/search?page=1&sort=date&q=rust",
        "title": "Search Results"
    });

    let response2 = server.post("/api/v1/content").json(&payload2).await;
    response2.assert_status_ok();
    let json_response2: Value = response2.json();
    assert_eq!(json_response2["id"].as_u64().unwrap(), first_id);

    // Verify only one record
    {
        use crate::common::test_utils;
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let all_items = test_utils::get_all_content_items(&mut conn);
        let saved_url = &all_items[0].url;

        // URL should have sorted query parameters
        assert!(saved_url.contains("page=1"));
        assert!(saved_url.contains("q=rust"));
        assert!(saved_url.contains("sort=date"));
    }
    Ok(())
}
