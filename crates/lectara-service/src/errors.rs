use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;
use tracing::error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("URL validation failed: {0}")]
    ValidationError(#[from] crate::validation::ValidationError),

    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),

    #[error("URL already exists with different metadata")]
    DuplicateUrlDifferentMetadata,

    #[error("Internal server error")]
    InternalError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ApiError::ValidationError(ref err) => (StatusCode::BAD_REQUEST, err.to_string()),
            ApiError::DuplicateUrlDifferentMetadata => (StatusCode::CONFLICT, self.to_string()),
            ApiError::DatabaseError(ref err) => {
                // Log the detailed error but don't expose it to the client
                error!(error = %err, "Database error occurred");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            ApiError::InternalError => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}
