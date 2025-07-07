use axum::http::StatusCode;
use lectara_service::models::NewContentItem;
use proptest::prelude::*;
use serde_json::{Value, json};

mod common;

// Generate arbitrary URLs with various normalizable features
prop_compose! {
    fn arb_normalizable_url()(
        base in "[a-z0-9]{3,10}\\.[a-z]{2,3}",
        path in prop::option::of("[a-z0-9/]{0,20}"),
        params in prop::collection::vec(
            ("[a-z]{1,5}", "[a-z0-9]{1,10}"),
            0..5
        ),
        fragment in prop::option::of("#[a-z0-9]{1,10}"),
        trailing_slash in prop::bool::ANY,
    ) -> String {
        format!(
            "https://{}{}{}{}{}",
            base,
            match path {
                Some(p) => format!("/{p}"),
                None => String::new(),
            },
            match trailing_slash {
                true => "/",
                false => "",
            },
            match params.is_empty() {
                false => format!(
                    "?{}",
                    params.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join("&")
                ),
                true => String::new(),
            },
            fragment.unwrap_or_default()
        )
    }
}

// Generate arbitrary content items
prop_compose! {
    fn arb_content_item()(
        url in arb_normalizable_url(),
        title in prop::option::of("[a-zA-Z0-9 ]{0,50}"),
        author in prop::option::of("[a-zA-Z ]{0,30}"),
        body in prop::option::of(prop::string::string_regex("[a-zA-Z0-9 \n]{0,500}").unwrap()),
    ) -> NewContentItem {
        NewContentItem {
            url,
            title: title.filter(|s| !s.trim().is_empty()),
            author: author.filter(|s| !s.trim().is_empty()),
            body: body.filter(|s| !s.trim().is_empty()),
        }
    }
}

#[cfg(test)]
mod properties {
    use super::*;
    use crate::common::server_utils::create_test_server;

    proptest! {
        #[test]
        fn idempotency_property(
            content in arb_content_item()
        ) {
            // Using tokio runtime for async
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, _db) = create_test_server();

                let payload = json!({
                    "url": content.url,
                    "title": content.title,
                    "author": content.author,
                    "body": content.body,
                });

                // First POST
                let response1 = server.post("/api/v1/content").json(&payload).await;
                prop_assert!(
                    response1.status_code() == StatusCode::OK ||
                    response1.status_code() == StatusCode::BAD_REQUEST
                );

                // If first succeeded, subsequent POSTs should be idempotent
                if response1.status_code() == StatusCode::OK {
                    for _ in 0..3 {
                        let response = server.post("/api/v1/content").json(&payload).await;
                        prop_assert_eq!(
                            response.status_code(),
                            StatusCode::OK,
                            "Repeated POST should return 200 OK"
                        );

                        let body1: Value = response1.json();
                        let body2: Value = response.json();
                        prop_assert_eq!(&body1["id"], &body2["id"]);
                    }
                }
                Ok(())
            }).expect("Async proptest should not fail")
        }

        #[test]
        fn url_normalization_property(
            base_url in "[a-z0-9]{3,10}\\.[a-z]{2,3}/[a-z0-9]{1,10}",
            params in prop::collection::btree_map("[a-z]{1,5}", "[a-z0-9]{1,10}", 1..5),
            fragment in prop::option::of("#[a-z0-9]{1,10}"),
            title in prop::option::of("[a-zA-Z0-9 ]{1,50}"),
        ) {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, _db) = create_test_server();

                // Create URLs with different orderings of query params (using unique keys)
                let params_vec1: Vec<(String, String)> = params.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                let mut params_vec2 = params_vec1.clone();
                params_vec2.reverse();

                let url1 = format!(
                    "https://{}?{}{}",
                    base_url,
                    params_vec1.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join("&"),
                    fragment.as_deref().unwrap_or("")
                );

                let url2 = format!(
                    "https://{}?{}",
                    base_url,
                    params_vec2.iter()
                        .map(|(k, v)| format!("{k}={v}"))
                        .collect::<Vec<_>>()
                        .join("&")
                );

                let payload1 = json!({
                    "url": url1,
                    "title": title,
                    "author": Value::Null,
                    "body": Value::Null,
                });

                let payload2 = json!({
                    "url": url2,
                    "title": title,
                    "author": Value::Null,
                    "body": Value::Null,
                });

                let response1 = server.post("/api/v1/content").json(&payload1).await;
                let response2 = server.post("/api/v1/content").json(&payload2).await;

                if response1.status_code() == StatusCode::OK {
                    prop_assert_eq!(
                        response2.status_code(),
                        StatusCode::OK,
                        "Same normalized URL should return OK on second post"
                    );

                    let body1: Value = response1.json();
                    let body2: Value = response2.json();
                    prop_assert_eq!(&body1["id"], &body2["id"]);
                    prop_assert_eq!(&body1["url"], &body2["url"]);
                }
                Ok(())
            }).expect("Async proptest should not fail")
        }

        #[test]
        fn conflict_detection_property(
            url in arb_normalizable_url(),
            title1 in "[a-zA-Z0-9 ]{1,50}",
            title2 in "[a-zA-Z0-9 ]{1,50}",
        ) {
            prop_assume!(title1 != title2);

            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let (server, _db) = create_test_server();

                let payload1 = json!({
                    "url": url,
                    "title": title1,
                    "author": Value::Null,
                    "body": Value::Null,
                });

                let payload2 = json!({
                    "url": url,
                    "title": title2,
                    "author": Value::Null,
                    "body": Value::Null,
                });

                let response1 = server.post("/api/v1/content").json(&payload1).await;

                if response1.status_code() == StatusCode::OK {
                    let response2 = server.post("/api/v1/content").json(&payload2).await;
                    prop_assert_eq!(
                        response2.status_code(),
                        StatusCode::CONFLICT,
                        "Different metadata for same URL should return 409"
                    );
                }
                Ok(())
            }).expect("Async proptest should not fail")
        }
    }
}
