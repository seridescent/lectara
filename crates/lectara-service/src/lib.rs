use axum::Router;
use diesel::sqlite::SqliteConnection;
use std::sync::{Arc, Mutex};

pub mod errors;
pub mod models;
pub mod routes;
pub mod schema;
pub mod shutdown;
pub mod validation;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<SqliteConnection>>,
}

pub fn create_app(state: AppState) -> Router {
    routes::create_router().with_state(state)
}
