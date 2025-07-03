use axum::{
    Router,
    extract::{Json, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, instrument, warn};

use crate::errors::ApiError;

pub mod errors;
pub mod models;
pub mod schema;
pub mod validation;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<SqliteConnection>>,
}

#[derive(Debug, serde::Deserialize)]
struct CreateContentRequest {
    url: String,
    title: Option<String>,
    author: Option<String>,
}

#[derive(Debug, Serialize)]
struct ContentResponse {
    id: u32,
}

async fn health() -> &'static str {
    "OK"
}

#[instrument(skip(state), fields(url = %payload.url, has_title = payload.title.is_some(), has_author = payload.author.is_some()))]
async fn add_content(
    State(state): State<AppState>,
    Json(payload): Json<CreateContentRequest>,
) -> Result<ResponseJson<ContentResponse>, ApiError> {
    use crate::schema::content_items;

    debug!("Processing content request");

    // Create and validate the content item
    let new_content = models::NewContentItem::new(payload.url, payload.title, payload.author)?;
    debug!(normalized_url = %new_content.url, "URL validated and normalized");

    let mut conn = state.db.lock().unwrap();

    // Check if URL already exists
    let existing_item = content_items::table
        .filter(content_items::url.eq(&new_content.url))
        .first::<models::ContentItem>(&mut *conn)
        .optional()?;

    if let Some(existing) = existing_item {
        // Check if metadata matches - if not, return error
        if existing.title != new_content.title || existing.author != new_content.author {
            warn!(
                existing_title = ?existing.title,
                new_title = ?new_content.title,
                existing_author = ?existing.author,
                new_author = ?new_content.author,
                "URL already exists with different metadata"
            );
            return Err(ApiError::DuplicateUrlDifferentMetadata);
        }

        // Return existing item (idempotent behavior)
        info!(id = existing.id, "Returning existing content item");
        let response = ContentResponse {
            id: existing.id as u32,
        };
        return Ok(ResponseJson(response));
    }

    // Insert new item
    let inserted_content = diesel::insert_into(content_items::table)
        .values(&new_content)
        .returning(content_items::all_columns)
        .get_result::<models::ContentItem>(&mut *conn)?;

    info!(
        id = inserted_content.id,
        "Successfully created new content item"
    );

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
