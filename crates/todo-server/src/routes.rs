use axum::{routing::get, Router};
use tower_http::{compression::CompressionLayer, trace::TraceLayer};

pub async fn create_router() -> anyhow::Result<Router> {
    let app = Router::new()
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new());

    Ok(app)
}

async fn health_check() -> &'static str {
    "OK"
}
