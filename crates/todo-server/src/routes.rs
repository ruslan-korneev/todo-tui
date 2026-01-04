use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::auth::auth_middleware;
use crate::handlers::auth as auth_handlers;
use crate::{Config, DbPool};

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Config,
}

pub fn create_router(db: DbPool, config: Config) -> Router {
    let state = AppState { db, config };

    // Public auth routes
    let auth_routes = Router::new()
        .route("/register", post(auth_handlers::register))
        .route("/login", post(auth_handlers::login))
        .route("/refresh", post(auth_handlers::refresh));

    // Protected auth routes
    let protected_auth_routes = Router::new()
        .route("/logout", post(auth_handlers::logout))
        .route("/me", get(auth_handlers::me))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine all routes
    Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1/auth", protected_auth_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}
