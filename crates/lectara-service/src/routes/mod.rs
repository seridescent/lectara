use crate::AppState;
use axum::Router;

pub mod api;
pub mod web;

pub fn create_router<S: AppState>() -> Router<S> {
    Router::new()
        .nest("/api", api::create_api_router())
        .nest("/web", web::create_web_router())
}

pub fn create_api_only_router<S: AppState>() -> Router<S> {
    Router::new().nest("/api", api::create_api_router())
}

pub fn create_api_v1_only_router<S: AppState>() -> Router<S> {
    Router::new().merge(api::v1::create_api_v1_router())
}
