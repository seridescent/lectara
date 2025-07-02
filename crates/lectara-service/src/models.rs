use crate::validation::normalize_url;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Serialize)]
#[diesel(table_name = crate::schema::content_items)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ContentItem {
    pub id: i32,
    pub url: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Insertable, Deserialize)]
#[diesel(table_name = crate::schema::content_items)]
pub struct NewContentItem {
    pub url: String,
    pub title: Option<String>,
    pub author: Option<String>,
}

impl NewContentItem {
    pub fn new(
        url: String,
        title: Option<String>,
        author: Option<String>,
    ) -> Result<Self, crate::validation::ValidationError> {
        let normalized_url = normalize_url(&url)?;

        Ok(NewContentItem {
            url: normalized_url,
            title,
            author,
        })
    }
}
