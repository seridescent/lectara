use axum::Router;
use diesel::sqlite::SqliteConnection;
use std::sync::{Arc, Mutex};

use crate::repositories::{ContentRepository, SqliteContentRepository};

pub mod errors;
pub mod models;
pub mod repositories;
pub mod routes;
pub mod schema;
pub mod shutdown;
pub mod validation;

#[derive(Clone)]
pub struct PocAppState {
    pub db: Arc<Mutex<SqliteConnection>>,
}

pub trait AppState: Clone + Send + Sync + 'static {
    type ContentRepo: ContentRepository;

    fn content_repo(&self) -> Self::ContentRepo;
}

#[derive(Clone)]
pub struct DefaultAppState {
    content_repository: SqliteContentRepository,
}

impl DefaultAppState {
    pub fn new(db: Arc<Mutex<SqliteConnection>>) -> Self {
        Self {
            content_repository: SqliteContentRepository::new(db),
        }
    }
}

impl AppState for DefaultAppState {
    type ContentRepo = SqliteContentRepository;

    fn content_repo(&self) -> Self::ContentRepo {
        self.content_repository.clone()
    }
}

pub fn create_app(state: PocAppState) -> Router {
    routes::create_router().with_state(state)
}
