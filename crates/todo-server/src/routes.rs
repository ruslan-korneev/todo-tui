use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

use crate::auth::auth_middleware;
use crate::handlers::{
    auth as auth_handlers, comments as comment_handlers, documents as document_handlers,
    search as search_handlers, statuses as status_handlers, tags as tag_handlers,
    tasks as task_handlers, workspaces as workspace_handlers,
};
use crate::{Config, DbPool};

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub config: Config,
}

pub fn create_router(db: DbPool, config: Config) -> Router {
    let state = AppState { db, config };

    // Public auth routes (no middleware)
    let public_auth_routes = Router::new()
        .route("/register", post(auth_handlers::register))
        .route("/login", post(auth_handlers::login))
        .route("/refresh", post(auth_handlers::refresh))
        .route("/verify-email", post(auth_handlers::verify_email))
        .route("/resend-verification", post(auth_handlers::resend_verification));

    // Protected auth routes (need auth)
    let protected_auth_routes = Router::new()
        .route("/logout", post(auth_handlers::logout))
        .route("/me", get(auth_handlers::me))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine auth routes - public first, then protected
    let auth_routes = Router::new()
        .merge(public_auth_routes)
        .merge(protected_auth_routes);

    // Workspace routes (all protected)
    let workspace_routes = Router::new()
        .route("/", post(workspace_handlers::create_workspace))
        .route("/", get(workspace_handlers::list_workspaces))
        .route("/:id", get(workspace_handlers::get_workspace))
        .route("/:id", patch(workspace_handlers::update_workspace))
        .route("/:id", delete(workspace_handlers::delete_workspace))
        .route("/:id/members", get(workspace_handlers::list_members))
        .route("/:id/invites", post(workspace_handlers::create_invite))
        .route(
            "/:id/members/:user_id",
            put(workspace_handlers::update_member_role).delete(workspace_handlers::remove_member),
        );

    // Status routes (nested under workspaces)
    let status_routes = Router::new()
        .route("/", get(status_handlers::list_statuses))
        .route("/", post(status_handlers::create_status))
        .route("/reorder", post(status_handlers::reorder_statuses))
        .route("/:status_id", patch(status_handlers::update_status))
        .route("/:status_id", delete(status_handlers::delete_status));

    // Task routes (nested under workspaces)
    let task_routes = Router::new()
        .route("/", get(task_handlers::list_tasks))
        .route("/", post(task_handlers::create_task))
        .route("/:task_id", get(task_handlers::get_task))
        .route("/:task_id", patch(task_handlers::update_task))
        .route("/:task_id", delete(task_handlers::delete_task))
        .route("/:task_id/move", post(task_handlers::move_task));

    // Comment routes (nested under tasks)
    let comment_routes = Router::new()
        .route("/", get(comment_handlers::list_comments))
        .route("/", post(comment_handlers::create_comment))
        .route("/:comment_id", patch(comment_handlers::update_comment))
        .route("/:comment_id", delete(comment_handlers::delete_comment));

    // Search routes (nested under workspaces)
    let search_routes = Router::new().route("/", get(search_handlers::search));

    // Tag routes (nested under workspaces)
    let tag_routes = Router::new()
        .route("/", get(tag_handlers::list_tags))
        .route("/", post(tag_handlers::create_tag))
        .route("/:tag_id", patch(tag_handlers::update_tag))
        .route("/:tag_id", delete(tag_handlers::delete_tag));

    // Task tag routes (nested under tasks)
    let task_tag_routes = Router::new()
        .route("/", get(tag_handlers::get_task_tags))
        .route("/", axum::routing::put(tag_handlers::set_task_tags));

    // Document routes (nested under workspaces)
    let document_routes = Router::new()
        .route("/", get(document_handlers::list_documents))
        .route("/", post(document_handlers::create_document))
        .route("/:doc_id", get(document_handlers::get_document))
        .route("/:doc_id", patch(document_handlers::update_document))
        .route("/:doc_id", delete(document_handlers::delete_document))
        .route("/:doc_id/move", post(document_handlers::move_document))
        // Task-Document linking
        .route(
            "/:doc_id/tasks",
            get(document_handlers::list_linked_tasks).post(document_handlers::link_task),
        )
        .route(
            "/:doc_id/tasks/:task_id",
            delete(document_handlers::unlink_task),
        );

    // Task linked documents route
    let task_documents_route = Router::new()
        .route("/", get(document_handlers::list_linked_documents));

    // Protected routes with auth middleware
    let protected_routes = Router::new()
        .nest("/workspaces", workspace_routes)
        .nest("/workspaces/:id/statuses", status_routes)
        .nest("/workspaces/:id/tasks", task_routes)
        .nest("/workspaces/:id/tasks/:task_id/comments", comment_routes)
        .nest("/workspaces/:id/tasks/:task_id/tags", task_tag_routes)
        .nest(
            "/workspaces/:id/tasks/:task_id/documents",
            task_documents_route,
        )
        .nest("/workspaces/:id/tags", tag_routes)
        .nest("/workspaces/:id/documents", document_routes)
        .nest("/workspaces/:id/search", search_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Public invite routes (view invite without auth)
    let public_invite_routes = Router::new()
        .route("/:token", get(workspace_handlers::get_invite));

    // Protected invite routes (accept invite requires auth)
    let protected_invite_routes = Router::new()
        .route("/:token/accept", post(workspace_handlers::accept_invite))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    // Combine all routes
    Router::new()
        .route("/health", get(health_check))
        .nest("/api/v1/auth", auth_routes)
        .nest("/api/v1/invites", public_invite_routes)
        .nest("/api/v1/invites", protected_invite_routes)
        .nest("/api/v1", protected_routes)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health_check() -> &'static str {
    "OK"
}
