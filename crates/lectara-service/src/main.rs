use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use lectara_service::{AppState, create_app};
use std::sync::{Arc, Mutex};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("lectara_service=debug".parse().unwrap()),
        )
        .init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");

    let connection = SqliteConnection::establish(&database_url).unwrap_or_else(|err| {
        error!(database_url = %database_url, error = %err, "Failed to connect to database");
        std::process::exit(1);
    });

    info!(database_url = %database_url, "Connected to database");

    let state = AppState {
        db: Arc::new(Mutex::new(connection)),
    };

    let app = create_app(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap_or_else(|err| {
            error!(bind_address = "0.0.0.0:3000", error = %err, "Failed to bind to address");
            std::process::exit(1);
        });

    info!("Server running on http://localhost:3000");

    if let Err(err) = axum::serve(listener, app).await {
        error!(error = %err, "Server error");
        std::process::exit(1);
    }
}
