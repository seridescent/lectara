use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

pub fn establish_test_connection() -> SqliteConnection {
    let mut connection =
        SqliteConnection::establish(":memory:").expect("Failed to create in-memory database");

    connection
        .run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    connection
}

pub mod test_utils {
    use super::*;
    use crate::models::ContentItem;
    use crate::schema::content_items;

    pub fn count_content_items(conn: &mut SqliteConnection) -> i64 {
        content_items::table
            .count()
            .get_result(conn)
            .expect("Failed to count content items")
    }

    pub fn get_all_content_items(conn: &mut SqliteConnection) -> Vec<ContentItem> {
        content_items::table
            .load::<ContentItem>(conn)
            .expect("Failed to load content items")
    }

    pub fn get_content_item_by_url(conn: &mut SqliteConnection, url: &str) -> Option<ContentItem> {
        content_items::table
            .filter(content_items::url.eq(url))
            .first::<ContentItem>(conn)
            .optional()
            .expect("Failed to query content item by URL")
    }
}
