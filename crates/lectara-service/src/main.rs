use diesel::Connection;
use diesel::sqlite::SqliteConnection;
use lectara_service::{
    DefaultAppState,
    routes::create_router,
    shutdown::{GracefulShutdownLayer, ShutdownState},
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{timeout::TimeoutLayer, trace::TraceLayer};
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

    let app_state = DefaultAppState::new(Arc::new(Mutex::new(connection)));
    let shutdown_state = ShutdownState::new();

    let app = create_router()
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(GracefulShutdownLayer::new(shutdown_state.clone()))
                .layer(TimeoutLayer::new(Duration::from_secs(15))),
        )
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap_or_else(|err| {
            error!(bind_address = "0.0.0.0:3000", error = %err, "Failed to bind to address");
            std::process::exit(1);
        });

    info!("Server running on http://localhost:3000");

    let server = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal(shutdown_state));

    if let Err(err) = server.await {
        error!(error = %err, "Server error");
        std::process::exit(1);
    }
}

async fn shutdown_signal(shutdown_state: ShutdownState) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, starting graceful shutdown");
    let shutdown_completed = shutdown_state.completed();
    shutdown_state.start_shutdown();

    shutdown_completed.await;
    info!("Graceful shutdown completed - all requests finished");
}
