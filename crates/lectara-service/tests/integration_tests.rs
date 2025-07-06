use anyhow::Result;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use hyper::Method;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use tower::{Service, ServiceExt};

mod common;

mod helpers {
    use super::*;
    use crate::common::establish_test_connection;
    use lectara_service::{PocAppState, create_app};

    pub fn create_test_app() -> (Router, Arc<Mutex<diesel::sqlite::SqliteConnection>>) {
        let connection = establish_test_connection();
        let db = Arc::new(Mutex::new(connection));

        let state = PocAppState { db: db.clone() };

        let app = create_app(state);
        (app, db)
    }

    pub async fn make_request(
        app: &mut Router,
        request: Request<Body>,
    ) -> Result<(StatusCode, Value)> {
        let response = ServiceExt::<Request<Body>>::ready(app)
            .await?
            .call(request)
            .await?;

        let status = response.status();
        let body_bytes = to_bytes(response.into_body(), usize::MAX).await?;
        let body_str = String::from_utf8(body_bytes.to_vec())?;

        let json_response: Value = if body_str.is_empty() || body_str == "\"OK\"" {
            json!(body_str.trim_matches('"'))
        } else {
            serde_json::from_str(&body_str).unwrap_or(json!(body_str))
        };

        Ok((status, json_response))
    }
}

#[tokio::test]
async fn test_health_endpoint() -> Result<()> {
    let (mut app, _db) = helpers::create_test_app();

    let request = Request::builder()
        .method("GET")
        .uri("/health")
        .body(Body::empty())?;

    let (status, response) = helpers::make_request(&mut app, request).await?;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(response, json!("OK"));
    Ok(())
}

#[tokio::test]
async fn test_add_content_endpoint() -> Result<()> {
    let (mut app, db) = helpers::create_test_app();

    let content_payload = json!({
        "url": "https://example.com/test-article",
        "title": "Test Article",
        "author": "Test Author"
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(content_payload.to_string()))?;

    let (status, response) = helpers::make_request(&mut app, request).await?;

    assert_eq!(status, StatusCode::OK);
    assert!(response["id"].is_number());

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
    let (mut app, db) = helpers::create_test_app();

    let content_payload = json!({
        "url": "https://example.com/minimal"
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(content_payload.to_string()))?;

    let (status, _) = helpers::make_request(&mut app, request).await?;

    assert_eq!(status, StatusCode::OK);

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
    let (mut app, db) = helpers::create_test_app();

    // Add first item
    let first_payload = json!({
        "url": "https://example.com/first",
        "title": "First Article"
    });

    let request1 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(first_payload.to_string()))?;

    helpers::make_request(&mut app, request1).await?;

    // Add second item
    let second_payload = json!({
        "url": "https://example.com/second",
        "author": "Second Author"
    });

    let request2 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(second_payload.to_string()))?;

    helpers::make_request(&mut app, request2).await?;

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
    let (mut app, db) = helpers::create_test_app();

    // Add first item
    let first_payload = json!({
        "url": "https://example.com/duplicate-test",
        "title": "First Title",
        "author": "First Author"
    });

    let request1 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(first_payload.to_string()))?;

    let (status1, response1) = helpers::make_request(&mut app, request1).await?;
    assert_eq!(status1, StatusCode::OK);
    let _first_id = response1["id"].as_u64().unwrap();

    // Attempt to add same URL again with different metadata
    let second_payload = json!({
        "url": "https://example.com/duplicate-test",
        "title": "Second Title",
        "author": "Second Author"
    });

    let request2 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(second_payload.to_string()))?;

    let (status2, _response2) = helpers::make_request(&mut app, request2).await?;

    // Should return conflict error for different metadata
    assert_eq!(status2, StatusCode::CONFLICT);

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
    let (mut app, db) = helpers::create_test_app();

    // Add first item
    let payload = json!({
        "url": "https://example.com/idempotent-test",
        "title": "Same Title",
        "author": "Same Author"
    });

    let request1 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))?;

    let (status1, response1) = helpers::make_request(&mut app, request1).await?;
    assert_eq!(status1, StatusCode::OK);
    let first_id = response1["id"].as_u64().unwrap();

    // Add same item again with identical metadata - should be idempotent
    let request2 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))?;

    let (status2, response2) = helpers::make_request(&mut app, request2).await?;

    // Should return existing record (truly idempotent)
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(response2["id"].as_u64().unwrap(), first_id);

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
    let (mut app, db) = helpers::create_test_app();

    // Add URL with fragment and trailing slash
    let payload1 = json!({
        "url": "https://example.com/article/#section1",
        "title": "Test Article"
    });

    let request1 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(payload1.to_string()))?;

    let (status1, response1) = helpers::make_request(&mut app, request1).await?;
    assert_eq!(status1, StatusCode::OK);
    let first_id = response1["id"].as_u64().unwrap();

    // Try same URL without fragment - should be treated as duplicate with same metadata
    let payload2 = json!({
        "url": "https://example.com/article/",
        "title": "Test Article"
    });

    let request2 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(payload2.to_string()))?;

    let (status2, response2) = helpers::make_request(&mut app, request2).await?;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(response2["id"].as_u64().unwrap(), first_id);

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
    let (mut app, _db) = helpers::create_test_app();

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

        let request = Request::builder()
            .method(Method::POST)
            .uri("/content")
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))?;

        let (status, _) = helpers::make_request(&mut app, request).await?;

        // Should reject with 400 Bad Request
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "Expected BAD_REQUEST for: {description}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn test_invalid_content_type() -> Result<()> {
    let (mut app, _db) = helpers::create_test_app();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "text/plain") // Wrong content type
        .body(Body::from(r#"{"url": "https://example.com"}"#))?;

    let (status, _) = helpers::make_request(&mut app, request).await?;

    // Axum should reject non-JSON content types
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    Ok(())
}

#[tokio::test]
async fn test_missing_content_type() -> Result<()> {
    let (mut app, _db) = helpers::create_test_app();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/content")
        // No content-type header
        .body(Body::from(r#"{"url": "https://example.com"}"#))?;

    let (status, _) = helpers::make_request(&mut app, request).await?;

    // Axum should reject missing content types for JSON extraction
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    Ok(())
}

#[tokio::test]
async fn test_url_with_query_parameters() -> Result<()> {
    let (mut app, db) = helpers::create_test_app();

    // URLs with different query parameter orders should normalize to same URL
    let payload1 = json!({
        "url": "https://example.com/search?q=rust&sort=date&page=1",
        "title": "Search Results"
    });

    let request1 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(payload1.to_string()))?;

    let (status1, response1) = helpers::make_request(&mut app, request1).await?;
    assert_eq!(status1, StatusCode::OK);
    let first_id = response1["id"].as_u64().unwrap();

    // Same parameters in different order with same metadata
    let payload2 = json!({
        "url": "https://example.com/search?page=1&sort=date&q=rust",
        "title": "Search Results"
    });

    let request2 = Request::builder()
        .method(Method::POST)
        .uri("/content")
        .header("content-type", "application/json")
        .body(Body::from(payload2.to_string()))?;

    let (status2, response2) = helpers::make_request(&mut app, request2).await?;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(response2["id"].as_u64().unwrap(), first_id);

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
