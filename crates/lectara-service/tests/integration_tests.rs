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
    use lectara_service::{AppState, create_app};

    pub fn create_test_app() -> (Router, Arc<Mutex<diesel::sqlite::SqliteConnection>>) {
        let connection = establish_test_connection();
        let db = Arc::new(Mutex::new(connection));

        let state = AppState { db: db.clone() };

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
    assert_eq!(response, json!("O"));
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
