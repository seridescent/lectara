use axum::Router;

pub fn create_web_router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
    // TODO: Add web app routes here
    // For example:
    // .route("/", get(index))
    // .route("/app/*path", get(serve_static))
}
