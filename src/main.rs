use axum::{
    Router,
    extract::Json,
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

mod models;
mod schema;

#[derive(Debug, Deserialize, Serialize)]
struct ContentItem {
    url: String,
    title: Option<String>,
    author: Option<String>,
}

#[derive(Debug, Serialize)]
struct ContentResponse {
    id: u32,
    message: String,
}

async fn health() -> &'static str {
    "OK"
}

async fn add_content(
    Json(payload): Json<ContentItem>,
) -> Result<ResponseJson<ContentResponse>, StatusCode> {
    println!("Received content: {:#?}", payload);

    let response = ContentResponse {
        id: 1,
        message: "Content saved successfully".to_string(),
    };

    Ok(ResponseJson(response))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(health))
        .route("/content", post(add_content));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");

    axum::serve(listener, app).await.unwrap();
}
