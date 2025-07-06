use crate::errors::ApiError;
use crate::models::{ContentItem, NewContentItem};
use async_trait::async_trait;

#[async_trait]
pub trait ContentRepository: Clone + Send + Sync + 'static {
    async fn find_by_url(&self, url: &str) -> Result<Option<ContentItem>, ApiError>;
    async fn create(&self, content: &NewContentItem) -> Result<ContentItem, ApiError>;
    async fn find_by_id(&self, id: i32) -> Result<Option<ContentItem>, ApiError>;
}
