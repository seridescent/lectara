use crate::AppState;
use axum::Router;

pub mod v1;

pub fn create_api_router<S: AppState>() -> Router<S> {
    Router::new().nest("/v1", v1::create_api_v1_router())
}
