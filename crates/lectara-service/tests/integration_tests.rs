use anyhow::Result;
use axum::http::StatusCode;
use serde_json::{Value, json};

mod common;

pub mod test_utils {
    use diesel::SqliteConnection;
    use diesel::prelude::*;
    use lectara_service::models::ContentItem;
    use lectara_service::schema::content_items;

    pub fn count_content_items(conn: &mut SqliteConnection) -> i64 {
        content_items::table
            .count()
            .get_result(conn)
            .expect("Failed to count content items")
    }

    pub fn get_all_content_items(conn: &mut SqliteConnection) -> Vec<ContentItem> {
        content_items::table
            .load::<ContentItem>(conn)
            .expect("Failed to load content items")
    }

    pub fn get_content_item_by_url(conn: &mut SqliteConnection, url: &str) -> Option<ContentItem> {
        content_items::table
            .filter(content_items::url.eq(url))
            .first::<ContentItem>(conn)
            .optional()
            .expect("Failed to query content item by URL")
    }
}

#[tokio::test]
async fn test_add_content_endpoint() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

    let content_payload = json!({
        "url": "https://example.com/test-article",
        "title": "Test Article",
        "author": "Test Author",
        "body": "This is the content body of the test article with some sample text."
    });

    let response = server.post("/api/v1/content").json(&content_payload).await;

    response.assert_status_ok();
    let json_response: Value = response.json();
    assert!(json_response["id"].is_number());

    // Verify database state
    {
        let mut conn = db.lock().unwrap();

        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/test-article")
                .expect("Content item should exist in database");

        assert_eq!(saved_item.url, "https://example.com/test-article");
        assert_eq!(saved_item.title, Some("Test Article".to_string()));
        assert_eq!(saved_item.author, Some("Test Author".to_string()));
        assert_eq!(
            saved_item.body,
            Some("This is the content body of the test article with some sample text.".to_string())
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_add_content_minimal_payload() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

    let content_payload = json!({
        "url": "https://example.com/minimal"
    });

    let response = server.post("/api/v1/content").json(&content_payload).await;

    response.assert_status_ok();

    // Verify database state
    {
        let mut conn = db.lock().unwrap();

        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/minimal")
                .expect("Content item should exist in database");

        assert_eq!(saved_item.url, "https://example.com/minimal");
        assert!(saved_item.title.is_none());
        assert!(saved_item.author.is_none());
        assert!(saved_item.body.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn test_empty_body_converts_to_none() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

    let content_payload = json!({
        "url": "https://example.com/empty-body",
        "title": "Article with Empty Body",
        "author": "Test Author",
        "body": ""
    });

    let response = server.post("/api/v1/content").json(&content_payload).await;

    response.assert_status_ok();

    // Verify database state - empty string should be stored as None
    {
        let mut conn = db.lock().unwrap();

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/empty-body")
                .expect("Content item should exist in database");

        assert_eq!(saved_item.url, "https://example.com/empty-body");
        assert_eq!(
            saved_item.title,
            Some("Article with Empty Body".to_string())
        );
        assert_eq!(saved_item.author, Some("Test Author".to_string()));
        assert!(saved_item.body.is_none()); // Empty string converted to None
    }
    Ok(())
}

#[tokio::test]
async fn test_body_mismatch_handling() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

    // Add first item with body
    let first_payload = json!({
        "url": "https://example.com/body-test",
        "title": "Test Article",
        "author": "Test Author",
        "body": "Original body content"
    });

    let response1 = server.post("/api/v1/content").json(&first_payload).await;
    response1.assert_status_ok();

    // Attempt to add same URL with different body
    let second_payload = json!({
        "url": "https://example.com/body-test",
        "title": "Test Article",
        "author": "Test Author",
        "body": "Different body content"
    });

    let response2 = server.post("/api/v1/content").json(&second_payload).await;
    response2.assert_status(StatusCode::CONFLICT);

    // Attempt to add same URL with no body (body mismatch)
    let third_payload = json!({
        "url": "https://example.com/body-test",
        "title": "Test Article",
        "author": "Test Author"
    });

    let response3 = server.post("/api/v1/content").json(&third_payload).await;
    response3.assert_status(StatusCode::CONFLICT);

    // Verify original item is unchanged
    {
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/body-test")
                .expect("Content item should exist in database");

        assert_eq!(saved_item.body, Some("Original body content".to_string()));
    }
    Ok(())
}

#[tokio::test]
async fn test_multiple_content_items() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

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
    let (server, db) = common::server_utils::create_test_server();

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
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/duplicate-test")
                .expect("Content item should exist in database");

        // Should keep original metadata
        assert_eq!(saved_item.title, Some("First Title".to_string()));
        assert_eq!(saved_item.author, Some("First Author".to_string()));
        assert!(saved_item.body.is_none());
    }
    Ok(())
}

#[tokio::test]
async fn test_true_idempotent_behavior() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

    // Add first item
    let payload = json!({
        "url": "https://example.com/idempotent-test",
        "title": "Same Title",
        "author": "Same Author",
        "body": "Same body content"
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
        let mut conn = db.lock().unwrap();
        assert_eq!(test_utils::count_content_items(&mut conn), 1);

        let saved_item =
            test_utils::get_content_item_by_url(&mut conn, "https://example.com/idempotent-test")
                .expect("Content item should exist in database");

        // Verify all fields are preserved
        assert_eq!(saved_item.title, Some("Same Title".to_string()));
        assert_eq!(saved_item.author, Some("Same Author".to_string()));
        assert_eq!(saved_item.body, Some("Same body content".to_string()));
    }
    Ok(())
}

#[tokio::test]
async fn test_url_normalization() -> Result<()> {
    let (server, db) = common::server_utils::create_test_server();

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
    let (server, _db) = common::server_utils::create_test_server();

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
    let (server, _db) = common::server_utils::create_test_server();

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
    let (server, _db) = common::server_utils::create_test_server();

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
    let (server, db) = common::server_utils::create_test_server();

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
