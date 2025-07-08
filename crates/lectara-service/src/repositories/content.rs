use super::traits::{ContentRepository, ListContentParams, ListContentResult};
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

    async fn list(&self, params: &ListContentParams) -> Result<ListContentResult, ApiError> {
        let mut conn = self.db.lock().unwrap();

        let limit = params.limit.unwrap_or(50).min(1000) as i64;

        let mut query = content_items::table.into_boxed();

        if let Some(since) = params.since {
            query = query.filter(content_items::created_at.ge(since));
        }
        if let Some(until) = params.until {
            query = query.filter(content_items::created_at.le(until));
        }

        if let Some(offset) = params.offset {
            query = query.offset(offset as i64);
        }

        query = query.order((content_items::created_at.desc(), content_items::id.desc()));

        let items = query.limit(limit).load::<ContentItem>(&mut *conn)?;

        let mut count_query = content_items::table.into_boxed();
        if let Some(since) = params.since {
            count_query = count_query.filter(content_items::created_at.ge(since));
        }
        if let Some(until) = params.until {
            count_query = count_query.filter(content_items::created_at.le(until));
        }
        let total = count_query.count().get_result::<i64>(&mut *conn)? as u64;

        Ok(ListContentResult { items, total })
    }
}
