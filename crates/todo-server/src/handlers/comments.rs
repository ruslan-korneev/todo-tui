use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use todo_shared::{
    api::{CreateCommentRequest, UpdateCommentRequest},
    CommentWithAuthor, WorkspaceRole,
};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::routes::AppState;

/// Helper to check workspace membership and return role
async fn check_membership(
    state: &AppState,
    workspace_id: Uuid,
    user_id: Uuid,
) -> Result<WorkspaceRole, AppError> {
    let role: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    role.map(|(r,)| r).ok_or(AppError::NotFound)
}

/// Helper to verify task belongs to workspace
async fn verify_task(
    state: &AppState,
    task_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    let exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM tasks WHERE id = $1 AND workspace_id = $2",
    )
    .bind(task_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

type CommentWithAuthorRow = (
    Uuid,                  // id
    Uuid,                  // task_id
    Uuid,                  // user_id
    String,                // author_username
    String,                // content
    chrono::DateTime<Utc>, // created_at
    chrono::DateTime<Utc>, // updated_at
);

fn row_to_comment(row: CommentWithAuthorRow) -> CommentWithAuthor {
    CommentWithAuthor {
        id: row.0,
        task_id: row.1,
        user_id: row.2,
        author_username: row.3,
        content: row.4,
        created_at: row.5,
        updated_at: row.6,
    }
}

/// GET /api/v1/workspaces/:id/tasks/:task_id/comments
pub async fn list_comments(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<CommentWithAuthor>>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;
    verify_task(&state, task_id, workspace_id).await?;

    let rows: Vec<CommentWithAuthorRow> = sqlx::query_as(
        r#"
        SELECT c.id, c.task_id, c.user_id, u.username, c.content, c.created_at, c.updated_at
        FROM task_comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.task_id = $1
        ORDER BY c.created_at ASC
        "#,
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await?;

    let comments = rows.into_iter().map(row_to_comment).collect();
    Ok(Json(comments))
}

/// POST /api/v1/workspaces/:id/tasks/:task_id/comments
pub async fn create_comment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<CreateCommentRequest>,
) -> Result<Json<CommentWithAuthor>, AppError> {
    // Any member can comment
    check_membership(&state, workspace_id, user.id).await?;
    verify_task(&state, task_id, workspace_id).await?;

    if req.content.trim().is_empty() {
        return Err(AppError::Validation("Comment content is required".to_string()));
    }

    // Get the user's username
    let username_row: Option<(String,)> =
        sqlx::query_as("SELECT username FROM users WHERE id = $1")
            .bind(user.id)
            .fetch_optional(&state.db)
            .await?;

    let (username,) = username_row.ok_or(AppError::NotFound)?;

    let id = Uuid::new_v4();
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO task_comments (id, task_id, user_id, content, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id)
    .bind(task_id)
    .bind(user.id)
    .bind(&req.content)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok(Json(CommentWithAuthor {
        id,
        task_id,
        user_id: user.id,
        author_username: username,
        content: req.content,
        created_at: now,
        updated_at: now,
    }))
}

/// PATCH /api/v1/workspaces/:id/tasks/:task_id/comments/:comment_id
pub async fn update_comment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id, comment_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(req): Json<UpdateCommentRequest>,
) -> Result<Json<CommentWithAuthor>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;
    verify_task(&state, task_id, workspace_id).await?;

    if req.content.trim().is_empty() {
        return Err(AppError::Validation("Comment content is required".to_string()));
    }

    // Verify comment exists and belongs to user (author only can edit)
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM task_comments WHERE id = $1 AND task_id = $2 AND user_id = $3",
    )
    .bind(comment_id)
    .bind(task_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_none() {
        return Err(AppError::Forbidden);
    }

    let now = Utc::now();

    sqlx::query(
        r#"
        UPDATE task_comments
        SET content = $1, updated_at = $2
        WHERE id = $3
        "#,
    )
    .bind(&req.content)
    .bind(now)
    .bind(comment_id)
    .execute(&state.db)
    .await?;

    // Fetch the updated comment with author
    let row: CommentWithAuthorRow = sqlx::query_as(
        r#"
        SELECT c.id, c.task_id, c.user_id, u.username, c.content, c.created_at, c.updated_at
        FROM task_comments c
        JOIN users u ON u.id = c.user_id
        WHERE c.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row_to_comment(row)))
}

/// DELETE /api/v1/workspaces/:id/tasks/:task_id/comments/:comment_id
pub async fn delete_comment(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id, comment_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<(), AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;
    verify_task(&state, task_id, workspace_id).await?;

    // Get comment to check ownership
    let comment: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM task_comments WHERE id = $1 AND task_id = $2",
    )
    .bind(comment_id)
    .bind(task_id)
    .fetch_optional(&state.db)
    .await?;

    let Some((comment_user_id,)) = comment else {
        return Err(AppError::NotFound);
    };

    // Author or admin can delete
    if comment_user_id != user.id && !role.can_admin() {
        return Err(AppError::Forbidden);
    }

    sqlx::query("DELETE FROM task_comments WHERE id = $1")
        .bind(comment_id)
        .execute(&state.db)
        .await?;

    Ok(())
}
