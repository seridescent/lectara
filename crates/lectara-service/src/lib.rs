use axum::{
    Router,
    extract::{Json, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::Serialize;
use std::sync::{Arc, Mutex};

pub mod models;
pub mod schema;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<SqliteConnection>>,
}

#[derive(Debug, Serialize)]
struct ContentResponse {
    id: u32,
}

async fn health() -> &'static str {
    "OK"
}

async fn add_content(
    State(state): State<AppState>,
    Json(payload): Json<models::NewContentItem>,
) -> Result<ResponseJson<ContentResponse>, StatusCode> {
    use crate::schema::content_items;

    println!("Received content: {payload:#?}");

    let mut conn = state.db.lock().unwrap();
    let inserted_content = diesel::insert_into(content_items::table)
        .values(&payload)
        .returning(content_items::all_columns)
        .get_result::<models::ContentItem>(&mut *conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = ContentResponse {
        id: inserted_content.id as u32,
    };

    Ok(ResponseJson(response))
}

pub fn create_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/content", post(add_content))
        .with_state(state)
}
