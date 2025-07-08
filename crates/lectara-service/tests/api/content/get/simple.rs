use crate::common::{server_utils::create_test_server, test_utils};
use anyhow::Result;
use axum::http::StatusCode;
use chrono::DateTime;
use serde_json::{Value, json};

#[tokio::test]
async fn test_list_content_empty_database() -> Result<()> {
    let (server, _db) = create_test_server();

    let response = server.get("/api/v1/content").await;
    response.assert_status_ok();

    let json_response: Value = response.json();
    assert_eq!(json_response["items"].as_array().unwrap().len(), 0);
    assert_eq!(json_response["total"].as_u64().unwrap(), 0);
    assert_eq!(json_response["limit"].as_u64().unwrap(), 50); // default limit

    Ok(())
}

#[tokio::test]
async fn test_get_content_by_id_not_found() -> Result<()> {
    let (server, _db) = create_test_server();

    let response = server.get("/api/v1/content/999").await;
    response.assert_status(StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn test_invalid_query_parameters() -> Result<()> {
    let (server, _db) = create_test_server();

    // Invalid limit
    let response = server.get("/api/v1/content?limit=-1").await;
    response.assert_status(StatusCode::BAD_REQUEST);

    // Invalid after_id
    let response = server.get("/api/v1/content?offset=not_a_number").await;
    response.assert_status(StatusCode::BAD_REQUEST);

    // Invalid datetime
    let response = server.get("/api/v1/content?since=not_a_date").await;
    response.assert_status(StatusCode::BAD_REQUEST);

    Ok(())
}

#[tokio::test]
async fn test_date_range_filtering() -> Result<()> {
    let (server, db) = create_test_server();

    // Create items with known timestamps
    let items = vec![
        (
            "2024-01-01T10:00:00Z",
            "https://example.com/item1",
            "Item 1",
        ),
        (
            "2024-01-02T10:00:00Z",
            "https://example.com/item2",
            "Item 2",
        ),
        (
            "2024-01-03T10:00:00Z",
            "https://example.com/item3",
            "Item 3",
        ),
    ];

    for (timestamp, url, title) in &items {
        let payload = json!({
            "url": url,
            "title": title,
        });

        let response = server.post("/api/v1/content").json(&payload).await;
        response.assert_status_ok();

        let json_response: Value = response.json();
        let item_id = json_response["id"].as_u64().unwrap() as i32;

        // Update the timestamp
        {
            let mut conn = db.lock().unwrap();
            let dt = DateTime::parse_from_rfc3339(timestamp).unwrap().naive_utc();
            test_utils::update_content_item_timestamp(&mut conn, item_id, dt);
        }
    }

    // Test since filter
    let response = server
        .get("/api/v1/content?since=2024-01-02T00:00:00Z")
        .await;
    response.assert_status_ok();

    let json_response: Value = response.json();
    let returned_items = json_response["items"].as_array().unwrap();
    assert_eq!(returned_items.len(), 2); // Items 2 and 3

    // Test until filter
    let response = server
        .get("/api/v1/content?until=2024-01-02T23:59:59Z")
        .await;
    response.assert_status_ok();

    let json_response: Value = response.json();
    let returned_items = json_response["items"].as_array().unwrap();
    assert_eq!(returned_items.len(), 2); // Items 1 and 2

    // Test range filter
    let response = server
        .get("/api/v1/content?since=2024-01-02T00:00:00Z&until=2024-01-02T23:59:59Z")
        .await;
    response.assert_status_ok();

    let json_response: Value = response.json();
    let returned_items = json_response["items"].as_array().unwrap();
    assert_eq!(returned_items.len(), 1); // Only Item 2

    Ok(())
}
