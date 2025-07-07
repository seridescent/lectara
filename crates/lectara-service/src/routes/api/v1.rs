use axum::{
    Router,
    extract::{Json, State},
    response::Json as ResponseJson,
    routing::post,
};
use serde::Serialize;
use tracing::{debug, info, instrument, warn};

use crate::errors::ApiError;
use crate::models;
use crate::{AppState, repositories::ContentRepository};

#[derive(Debug, serde::Deserialize)]
struct AddContentRequest {
    url: String,
    title: Option<String>,
    author: Option<String>,
    body: Option<String>,
}

#[derive(Debug, Serialize)]
struct ContentResponse {
    id: u32,
}

#[instrument(skip_all, fields(url = %payload.url, has_title = payload.title.is_some(), has_author = payload.author.is_some(), has_body = payload.body.is_some()))]
async fn add_content<S: AppState>(
    State(state): State<S>,
    Json(payload): Json<AddContentRequest>,
) -> Result<ResponseJson<ContentResponse>, ApiError> {
    debug!("Processing content request");

    // Create and validate the content item
    // Convert empty strings to None for body field
    let body = payload.body.filter(|s| !s.trim().is_empty());
    let new_content =
        models::NewContentItem::new(payload.url, payload.title, payload.author, body)?;
    debug!(normalized_url = %new_content.url, "URL validated and normalized");

    let content_repo = state.content_repo();

    // Check if URL already exists
    let existing_item = content_repo.find_by_url(&new_content.url).await?;

    if let Some(existing) = existing_item {
        // Check if metadata matches - if not, return error
        if existing.title != new_content.title {
            warn!(
                existing_title = ?existing.title,
                new_title = ?new_content.title,
                "URL already exists with different title"
            );
            return Err(ApiError::DuplicateUrlDifferentMetadata);
        }

        if existing.author != new_content.author {
            warn!(
                existing_author = ?existing.author,
                new_author = ?new_content.author,
                "URL already exists with different author"
            );
            return Err(ApiError::DuplicateUrlDifferentMetadata);
        }

        if existing.body != new_content.body {
            warn!(
                existing_body_length = existing.body.as_ref().map(|b| b.len()),
                new_body_length = new_content.body.as_ref().map(|b| b.len()),
                "URL already exists with different body content"
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

pub fn create_api_v1_router<S: AppState>() -> Router<S> {
    Router::new().route("/content", post(add_content::<S>))
}
