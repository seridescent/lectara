use diesel::{Connection, sqlite::SqliteConnection};
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

pub mod server_utils {
    use super::*;
    use axum_test::TestServer;
    use lectara_service::{DefaultAppState, routes};
    use std::sync::{Arc, Mutex};

    pub fn create_test_server() -> (TestServer, Arc<Mutex<SqliteConnection>>) {
        let connection = establish_test_connection();
        let db = Arc::new(Mutex::new(connection));

        let state = DefaultAppState::new(db.clone());
        let app = routes::create_router().with_state(state);

        let server = TestServer::new(app).unwrap();
        (server, db)
    }
}

pub mod test_utils {
    use diesel::SqliteConnection;
    use diesel::prelude::*;
    use lectara_service::models::ContentItem;
    use lectara_service::schema::content_items;

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

    #[allow(dead_code)]
    pub fn get_content_item_by_id(conn: &mut SqliteConnection, id: i32) -> Option<ContentItem> {
        content_items::table
            .find(id)
            .first::<ContentItem>(conn)
            .optional()
            .expect("Failed to query content item by ID")
    }

    pub fn update_content_item_timestamp(
        conn: &mut SqliteConnection,
        id: i32,
        timestamp: chrono::NaiveDateTime,
    ) {
        diesel::sql_query("UPDATE content_items SET created_at = ?1 WHERE id = ?2")
            .bind::<diesel::sql_types::Timestamp, _>(timestamp)
            .bind::<diesel::sql_types::Integer, _>(id)
            .execute(conn)
            .expect("Failed to update timestamp");
    }
}
