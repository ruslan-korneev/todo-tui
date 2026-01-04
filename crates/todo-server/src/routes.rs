use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::auth::auth_middleware;
use crate::handlers::{auth as auth_handlers, workspaces as workspace_handlers};
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
        .route("/me", get(auth_handlers::me));

    // Workspace routes (all protected)
    let workspace_routes = Router::new()
        .route("/", post(workspace_handlers::create_workspace))
        .route("/", get(workspace_handlers::list_workspaces))
        .route("/:id", get(workspace_handlers::get_workspace))
        .route("/:id", patch(workspace_handlers::update_workspace))
        .route("/:id", delete(workspace_handlers::delete_workspace));

    // Protected routes with auth middleware
    let protected_routes = Router::new()
        .nest("/auth", protected_auth_routes)
        .nest("/workspaces", workspace_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine all routes
    Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1", protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}
