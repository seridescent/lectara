use super::traits::ContentRepository;
use crate::errors::ApiError;
use crate::models::{ContentItem, NewContentItem};
use crate::schema::content_items;
use async_trait::async_trait;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct SqliteContentRepository {
    db: Arc<Mutex<SqliteConnection>>,
}

impl SqliteContentRepository {
    pub fn new(db: Arc<Mutex<SqliteConnection>>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ContentRepository for SqliteContentRepository {
    async fn find_by_url(&self, url: &str) -> Result<Option<ContentItem>, ApiError> {
        let mut conn = self.db.lock().unwrap();
        let result = content_items::table
            .filter(content_items::url.eq(url))
            .first::<ContentItem>(&mut *conn)
            .optional()?;
        Ok(result)
    }

    async fn create(&self, content: &NewContentItem) -> Result<ContentItem, ApiError> {
        let mut conn = self.db.lock().unwrap();
        let result = diesel::insert_into(content_items::table)
            .values(content)
            .returning(content_items::all_columns)
            .get_result::<ContentItem>(&mut *conn)?;
        Ok(result)
    }

    async fn find_by_id(&self, id: i32) -> Result<Option<ContentItem>, ApiError> {
        let mut conn = self.db.lock().unwrap();
        let result = content_items::table
            .find(id)
            .first::<ContentItem>(&mut *conn)
            .optional()?;
        Ok(result)
    }
}
