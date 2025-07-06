use axum::{
    Router,
    extract::{Json, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use serde::Serialize;
use tracing::{debug, info, instrument, warn};

use crate::errors::ApiError;
use crate::models;
use crate::{AppState, repositories::ContentRepository};

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
async fn add_content<S: AppState>(
    State(state): State<S>,
    Json(payload): Json<CreateContentRequest>,
) -> Result<ResponseJson<ContentResponse>, ApiError> {
    debug!("Processing content request");

    // Create and validate the content item
    let new_content = models::NewContentItem::new(payload.url, payload.title, payload.author)?;
    debug!(normalized_url = %new_content.url, "URL validated and normalized");

    let content_repo = state.content_repo();

    // Check if URL already exists
    let existing_item = content_repo.find_by_url(&new_content.url).await?;

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
    let inserted_content = content_repo.create(&new_content).await?;

    info!(
        id = inserted_content.id,
        "Successfully created new content item"
    );

    let response = ContentResponse {
        id: inserted_content.id as u32,
    };

    Ok(ResponseJson(response))
}

pub fn create_router<S: AppState>() -> Router<S> {
    Router::new()
        .route("/health", get(health))
        .route("/content", post(add_content::<S>))
}
