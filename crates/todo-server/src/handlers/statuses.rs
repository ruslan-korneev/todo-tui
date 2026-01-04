use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use serde::Deserialize;
use todo_shared::{
    api::{CreateStatusRequest, UpdateStatusRequest},
    TaskStatus, WorkspaceRole,
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

/// GET /api/v1/workspaces/:id/statuses
pub async fn list_statuses(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<TaskStatus>>, AppError> {
    // Verify membership
    check_membership(&state, workspace_id, user.id).await?;

    let rows: Vec<(Uuid, Uuid, String, String, Option<String>, i32, bool)> = sqlx::query_as(
        r#"
        SELECT id, workspace_id, name, slug, color, position, is_done
        FROM task_statuses
        WHERE workspace_id = $1
        ORDER BY position
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await?;

    let statuses = rows
        .into_iter()
        .map(|(id, workspace_id, name, slug, color, position, is_done)| TaskStatus {
            id,
            workspace_id,
            name,
            slug,
            color,
            position,
            is_done,
        })
        .collect();

    Ok(Json(statuses))
}

/// POST /api/v1/workspaces/:id/statuses
pub async fn create_status(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<CreateStatusRequest>,
) -> Result<Json<TaskStatus>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Status name is required".to_string()));
    }

    let id = Uuid::new_v4();
    let slug = req
        .name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    // Get max position
    let (max_pos,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(MAX(position), -1) FROM task_statuses WHERE workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await?;

    let position = max_pos + 1;
    let now = Utc::now();

    sqlx::query(
        r#"
        INSERT INTO task_statuses (id, workspace_id, name, slug, color, position, is_done, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&req.color)
    .bind(position)
    .bind(req.is_done)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok(Json(TaskStatus {
        id,
        workspace_id,
        name: req.name,
        slug,
        color: req.color,
        position,
        is_done: req.is_done,
    }))
}

/// PATCH /api/v1/workspaces/:id/statuses/:status_id
pub async fn update_status(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, status_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateStatusRequest>,
) -> Result<Json<TaskStatus>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    // Verify status belongs to workspace
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM task_statuses WHERE id = $1 AND workspace_id = $2",
    )
    .bind(status_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_none() {
        return Err(AppError::NotFound);
    }

    let row: (Uuid, Uuid, String, String, Option<String>, i32, bool) = sqlx::query_as(
        r#"
        UPDATE task_statuses
        SET name = COALESCE($1, name),
            color = COALESCE($2, color),
            is_done = COALESCE($3, is_done)
        WHERE id = $4
        RETURNING id, workspace_id, name, slug, color, position, is_done
        "#,
    )
    .bind(&req.name)
    .bind(&req.color)
    .bind(req.is_done)
    .bind(status_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(TaskStatus {
        id: row.0,
        workspace_id: row.1,
        name: row.2,
        slug: row.3,
        color: row.4,
        position: row.5,
        is_done: row.6,
    }))
}

/// DELETE /api/v1/workspaces/:id/statuses/:status_id
pub async fn delete_status(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, status_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_admin() {
        return Err(AppError::Forbidden);
    }

    // Check if there are tasks in this status
    let task_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM tasks WHERE status_id = $1",
    )
    .bind(status_id)
    .fetch_one(&state.db)
    .await?;

    if task_count.0 > 0 {
        return Err(AppError::Conflict(
            "Cannot delete status with existing tasks. Move or delete tasks first.".to_string(),
        ));
    }

    // Verify status belongs to workspace and delete
    let result = sqlx::query(
        "DELETE FROM task_statuses WHERE id = $1 AND workspace_id = $2",
    )
    .bind(status_id)
    .bind(workspace_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ReorderStatusesRequest {
    pub status_ids: Vec<Uuid>,
}

/// POST /api/v1/workspaces/:id/statuses/reorder
pub async fn reorder_statuses(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<ReorderStatusesRequest>,
) -> Result<Json<Vec<TaskStatus>>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    // Update positions in a transaction
    let mut tx = state.db.begin().await?;

    for (position, status_id) in req.status_ids.iter().enumerate() {
        sqlx::query(
            "UPDATE task_statuses SET position = $1 WHERE id = $2 AND workspace_id = $3",
        )
        .bind(position as i32)
        .bind(status_id)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    // Return updated list
    let rows: Vec<(Uuid, Uuid, String, String, Option<String>, i32, bool)> = sqlx::query_as(
        r#"
        SELECT id, workspace_id, name, slug, color, position, is_done
        FROM task_statuses
        WHERE workspace_id = $1
        ORDER BY position
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await?;

    let statuses = rows
        .into_iter()
        .map(|(id, workspace_id, name, slug, color, position, is_done)| TaskStatus {
            id,
            workspace_id,
            name,
            slug,
            color,
            position,
            is_done,
        })
        .collect();

    Ok(Json(statuses))
}
