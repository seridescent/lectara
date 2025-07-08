use axum::{
    Router,
    extract::{Json, Path, Query, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use chrono::{DateTime, NaiveDateTime};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

use crate::errors::ApiError;
use crate::models;
use crate::{
    AppState,
    repositories::{ContentRepository, ListContentParams},
};

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

#[derive(Debug, Deserialize)]
struct ListContentQuery {
    limit: Option<u32>,
    offset: Option<u32>,
    since: Option<String>, // ISO 8601 datetime string
    until: Option<String>, // ISO 8601 datetime string
}

#[derive(Debug, Serialize)]
struct ContentSummary {
    id: i32,
    url: String,
    title: Option<String>,
    author: Option<String>,
    created_at: NaiveDateTime,
}

#[derive(Debug, Serialize)]
struct ListContentResponse {
    items: Vec<ContentSummary>,
    total: u64,
    limit: u32,
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

#[instrument(skip_all, fields(limit = query.limit, offset = query.offset, has_since = query.since.is_some(), has_until = query.until.is_some()))]
async fn list_content<S: AppState>(
    State(state): State<S>,
    Query(query): Query<ListContentQuery>,
) -> Result<ResponseJson<ListContentResponse>, ApiError> {
    debug!("Processing list content request");

    // Parse datetime strings
    let since = if let Some(since_str) = &query.since {
        Some(
            DateTime::parse_from_rfc3339(since_str)
                .map_err(|_| {
                    ApiError::BadRequest(
                        "Invalid 'since' datetime format. Use RFC3339 format.".to_string(),
                    )
                })?
                .naive_utc(),
        )
    } else {
        None
    };

    let until = if let Some(until_str) = &query.until {
        Some(
            DateTime::parse_from_rfc3339(until_str)
                .map_err(|_| {
                    ApiError::BadRequest(
                        "Invalid 'until' datetime format. Use RFC3339 format.".to_string(),
                    )
                })?
                .naive_utc(),
        )
    } else {
        None
    };

    // Validate limit
    if let Some(limit) = query.limit {
        if limit == 0 {
            return Err(ApiError::BadRequest(
                "Limit must be greater than 0".to_string(),
            ));
        }
    }

    let params = ListContentParams {
        limit: query.limit,
        offset: query.offset,
        since,
        until,
    };

    let content_repo = state.content_repo();
    let result = content_repo.list(&params).await?;

    let items = result
        .items
        .into_iter()
        .map(|item| ContentSummary {
            id: item.id,
            url: item.url,
            title: item.title,
            author: item.author,
            created_at: item.created_at,
        })
        .collect();

    let response = ListContentResponse {
        items,
        total: result.total,
        limit: params.limit.unwrap_or(50),
    };

    info!(
        returned_count = response.items.len(),
        total = response.total,
        "Successfully retrieved content list"
    );

    Ok(ResponseJson(response))
}

#[instrument(skip_all, fields(id = %id))]
async fn get_content_by_id<S: AppState>(
    State(state): State<S>,
    Path(id): Path<i32>,
) -> Result<ResponseJson<models::ContentItem>, ApiError> {
    debug!("Processing get content by ID request");

    let content_repo = state.content_repo();
    let content = content_repo.find_by_id(id).await?;

    match content {
        Some(item) => {
            info!(id = item.id, "Successfully retrieved content item");
            Ok(ResponseJson(item))
        }
        None => {
            debug!("Content item not found");
            Err(ApiError::NotFound)
        }
    }
}

pub fn create_api_v1_router<S: AppState>() -> Router<S> {
    Router::new()
        .route("/content", post(add_content::<S>).get(list_content::<S>))
        .route("/content/{id}", get(get_content_by_id::<S>))
}
