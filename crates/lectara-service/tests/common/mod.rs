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
