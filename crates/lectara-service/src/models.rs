use crate::validation::normalize_url;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Queryable, Selectable, Serialize)]
#[diesel(table_name = crate::schema::content_items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ContentItem {
    pub id: i32,
    pub url: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub body: Option<String>,
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = crate::schema::content_items)]
pub struct NewContentItem {
    pub url: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub body: Option<String>,
}

impl NewContentItem {
    pub fn new(
        url: String,
        title: Option<String>,
        author: Option<String>,
        body: Option<String>,
    ) -> Result<Self, crate::validation::ValidationError> {
        let normalized_url = normalize_url(&url)?;

        Ok(NewContentItem {
            url: normalized_url,
            title,
            author,
            body,
        })
    }
}
