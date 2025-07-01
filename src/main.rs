use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use lectara::{AppState, create_app};
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "data/dev.db".to_string());

    let connection =
        SqliteConnection::establish(&database_url).expect("Failed to connect to database");

    let state = AppState {
        db: Arc::new(Mutex::new(connection)),
    };

    let app = create_app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://localhost:3000");

    axum::serve(listener, app).await.unwrap();
}
