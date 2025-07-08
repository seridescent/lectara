use chrono::{DateTime, Utc};
use proptest::prelude::*;
use url::form_urlencoded;

// Generate datetime ranges for testing date filtering
prop_compose! {
    fn arb_datetime_range()(
        start_secs in 1_600_000_000i64..1_700_000_000i64, // 2020-2023 range
        duration_secs in 1i64..86400 * 30, // 1 second to 30 days
    ) -> (DateTime<Utc>, DateTime<Utc>) {
        let start = DateTime::from_timestamp(start_secs, 0).unwrap();
        let end = DateTime::from_timestamp(start_secs + duration_secs, 0).unwrap();
        (start, end)
    }
}

// Generate content items with specific timestamps
prop_compose! {
    fn arb_content_with_timestamp()(
        timestamp in 1_600_000_000i64..1_700_000_000i64,
        url_suffix in "[a-z0-9]{3,10}",
        title in prop::option::of("[a-zA-Z0-9 ]{1,50}"),
        author in prop::option::of("[a-zA-Z ]{1,30}"),
        body in prop::option::of("[a-zA-Z0-9 ]{1,100}"),
    ) -> (i64, String, Option<String>, Option<String>, Option<String>) {
        (
            timestamp,
            format!("https://example.com/{url_suffix}"),
            title.filter(|s| !s.trim().is_empty()),
            author.filter(|s| !s.trim().is_empty()),
            body.filter(|s| !s.trim().is_empty()),
        )
    }
}

#[cfg(test)]
mod get_properties {
    use chrono::NaiveDateTime;
    use http::StatusCode;
    use serde_json::{Value, json};

    use crate::common::{server_utils::create_test_server, test_utils};

    use super::*;

    proptest! {
        #[test]
        fn list_content_ordering_property(
            mut items in prop::collection::vec(arb_content_with_timestamp(), 3..10),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, db) = create_test_server();

                // Sort items by timestamp for expected ordering
                items.sort_by_key(|(timestamp, _, _, _, _)| *timestamp);

                // Insert all items (in random order to test sorting)
                for (timestamp, url, title, author, body) in &items {
                    let payload = json!({
                        "url": url,
                        "title": title,
                        "author": author,
                        "body": body,
                    });

                    let response = server.post("/api/v1/content").json(&payload).await;
                    prop_assert_eq!(response.status_code(), StatusCode::OK);

                    let json_response: Value = response.json();
                    let item_id = json_response["id"].as_u64().unwrap() as i32;

                    // Update the created_at timestamp to our test timestamp
                    {
                        let mut conn = db.lock().unwrap();
                        let naive_dt = DateTime::from_timestamp(*timestamp, 0).unwrap().naive_utc();
                        test_utils::update_content_item_timestamp(&mut conn, item_id, naive_dt);
                    }
                }

                // Test that GET /content returns items in reverse chronological order
                let response = server.get("/api/v1/content").await;
                prop_assert_eq!(response.status_code(), StatusCode::OK);

                let json_response: Value = response.json();
                let returned_items = json_response["items"].as_array().unwrap();

                // Should return all items
                prop_assert_eq!(returned_items.len(), items.len());

                // Check ordering (newest first)
                for i in 1..returned_items.len() {
                    let prev_created = returned_items[i-1]["created_at"].as_str().unwrap();
                    let curr_created = returned_items[i]["created_at"].as_str().unwrap();

                    let prev_dt = NaiveDateTime::parse_from_str(prev_created, "%Y-%m-%dT%H:%M:%S").unwrap();
                    let curr_dt = NaiveDateTime::parse_from_str(curr_created, "%Y-%m-%dT%H:%M:%S").unwrap();

                    prop_assert!(prev_dt >= curr_dt, "Items should be ordered newest first");
                }

                Ok(())
            }).expect("Async proptest should not fail")
        }

        #[test]
        fn date_filtering_property(
            (start_date, end_date) in arb_datetime_range(),
            items_before in prop::collection::vec(arb_content_with_timestamp(), 1..5),
            items_in_range in prop::collection::vec(arb_content_with_timestamp(), 1..5),
            items_after in prop::collection::vec(arb_content_with_timestamp(), 1..5),
        )  {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, db) = create_test_server();

                let start_timestamp = start_date.timestamp();
                let end_timestamp = end_date.timestamp();

                // Create items before, in range, and after
                let mut all_items = Vec::new();

                for (i, (_, url, title, author, body)) in items_before.iter().enumerate() {
                    let timestamp = start_timestamp - 86400 - (i as i64); // Before range
                    all_items.push((timestamp, url.clone(), title.clone(), author.clone(), body.clone()));
                }

                for (i, (_, url, title, author, body)) in items_in_range.iter().enumerate() {
                    let timestamp = start_timestamp + (i as i64) * 60; // In range
                    all_items.push((timestamp, url.clone(), title.clone(), author.clone(), body.clone()));
                }

                for (i, (_, url, title, author, body)) in items_after.iter().enumerate() {
                    let timestamp = end_timestamp + 86400 + (i as i64); // After range
                    all_items.push((timestamp, url.clone(), title.clone(), author.clone(), body.clone()));
                }

                // Insert all items
                for (timestamp, url, title, author, body) in &all_items {
                    let payload = json!({
                        "url": url,
                        "title": title,
                        "author": author,
                        "body": body,
                    });

                    let response = server.post("/api/v1/content").json(&payload).await;
                    prop_assert_eq!(response.status_code(), StatusCode::OK);

                    let json_response: Value = response.json();
                    let item_id = json_response["id"].as_u64().unwrap() as i32;

                    // Update timestamp
                    {
                        let mut conn = db.lock().unwrap();
                        let naive_dt = DateTime::from_timestamp(*timestamp, 0).unwrap().naive_utc();
                        test_utils::update_content_item_timestamp(&mut conn, item_id, naive_dt);
                    }
                }

                // Test filtering with since parameter
                let since_param = form_urlencoded::byte_serialize(start_date.to_rfc3339().as_bytes()).collect::<String>();
                let response = server
                    .get(&format!("/api/v1/content?since={since_param}"))
                    .await;
                prop_assert_eq!(response.status_code(), StatusCode::OK,
                    "GET /api/v1/content?since={} failed with {}", since_param, response.text());

                let json_response: Value = response.json();
                let returned_items = json_response["items"].as_array().unwrap();

                // Should only return items from start_date onwards
                let expected_count = items_in_range.len() + items_after.len();
                prop_assert_eq!(returned_items.len(), expected_count);

                // Verify all returned items are after start_date
                for item in returned_items {
                    let created_at = item["created_at"].as_str().unwrap();
                    let item_dt = NaiveDateTime::parse_from_str(created_at, "%Y-%m-%dT%H:%M:%S").unwrap();
                    let item_timestamp = item_dt.and_utc().timestamp();
                    prop_assert!(item_timestamp >= start_timestamp);
                }

                Ok(())
            }).expect("async proptest failed")
        }

        #[test]
        fn pagination_consistency_property(
            items in prop::collection::vec(arb_content_with_timestamp(), 5..15),
            limit in 1usize..5,
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, db) = create_test_server();

                // Insert all items, track which ones were actually created
                let mut created_count = 0;
                for (timestamp, url, title, author, body) in &items {
                    let payload = json!({
                        "url": url,
                        "title": title,
                        "author": author,
                        "body": body,
                    });

                    let response = server.post("/api/v1/content").json(&payload).await;
                    // Might be 409 due to duplicate URL, which is OK
                    if response.status_code() == StatusCode::OK {
                        created_count += 1;
                        let json_response: Value = response.json();
                        let item_id = json_response["id"].as_u64().unwrap() as i32;

                        // Update timestamp
                        {
                            let mut conn = db.lock().unwrap();
                            let naive_dt = DateTime::from_timestamp(*timestamp, 0).unwrap().naive_utc();
                            test_utils::update_content_item_timestamp(&mut conn, item_id, naive_dt);
                        }
                    }
                }

                // Get first page
                let response = server
                    .get(&format!("/api/v1/content?limit={limit}"))
                    .await;
                prop_assert_eq!(response.status_code(), StatusCode::OK);

                let json_response: Value = response.json();
                let first_page = json_response["items"].as_array().unwrap();
                println!("first_page: {first_page:#?}");

                prop_assert!(first_page.len() <= limit);
                prop_assert_eq!(json_response["total"].as_u64().unwrap(), created_count as u64);

                if created_count > limit && !first_page.is_empty() {
                    let response2 = server
                        .get(&format!("/api/v1/content?offset={limit}&limit={limit}"))
                        .await;
                    prop_assert_eq!(response2.status_code(), StatusCode::OK);

                    let json_response2: Value = response2.json();
                    let second_page = json_response2["items"].as_array().unwrap();
                    println!("second_page: {second_page:#?}");

                    // Should not have any overlapping items
                    let first_page_ids: Vec<u64> = first_page.iter()
                        .map(|item| item["id"].as_u64().unwrap())
                        .collect();
                    let second_page_ids: Vec<u64> = second_page.iter()
                        .map(|item| item["id"].as_u64().unwrap())
                        .collect();

                    for id in &second_page_ids {
                        prop_assert!(!first_page_ids.contains(id),
                        "Pages (1: {:?}, 2: {:?}) should not overlap, but {:?} contains {}",
                        first_page_ids, second_page_ids, first_page_ids, id);
                    }
                }

                Ok(())
            }).expect("async proptest failed");
        }

        #[test]
        fn individual_item_retrieval_property(
            items in prop::collection::vec(arb_content_with_timestamp(), 1..10),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, _db) = create_test_server();

                let mut item_ids = Vec::new();

                // Insert all items and collect their IDs
                for (_, url, title, author, body) in &items {
                    let payload = json!({
                        "url": url,
                        "title": title,
                        "author": author,
                        "body": body,
                    });

                    let response = server.post("/api/v1/content").json(&payload).await;
                    prop_assert_eq!(response.status_code(), StatusCode::OK);

                    let json_response: Value = response.json();
                    item_ids.push(json_response["id"].as_u64().unwrap());
                }

                // Test retrieving each item individually
                for (i, item_id) in item_ids.iter().enumerate() {
                    let response = server
                        .get(&format!("/api/v1/content/{item_id}"))
                        .await;
                    prop_assert_eq!(response.status_code(), StatusCode::OK);

                    let json_response: Value = response.json();
                    prop_assert_eq!(json_response["id"].as_u64().unwrap(), *item_id);
                    prop_assert_eq!(json_response["url"].as_str().unwrap(), &items[i].1);

                    // Verify optional fields match
                    if let Some(ref title) = items[i].2 {
                        prop_assert_eq!(json_response["title"].as_str().unwrap(), title);
                    } else {
                        prop_assert!(json_response["title"].is_null());
                    }

                    if let Some(ref author) = items[i].3 {
                        prop_assert_eq!(json_response["author"].as_str().unwrap(), author);
                    } else {
                        prop_assert!(json_response["author"].is_null());
                    }

                    if let Some(ref body) = items[i].4 {
                        prop_assert_eq!(json_response["body"].as_str().unwrap(), body);
                    } else {
                        prop_assert!(json_response["body"].is_null());
                    }
                }

                // Test 404 for non-existent item
                let max_id = item_ids.iter().max().unwrap();
                let response = server
                    .get(&format!("/api/v1/content/{}", max_id + 1000))
                    .await;
                prop_assert_eq!(response.status_code(), StatusCode::NOT_FOUND);

                Ok(())
            }).expect("async proptest failed");
        }
    }
}
