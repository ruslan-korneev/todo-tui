use axum::{
    extract::{Path, State},
    Extension, Json,
};
use todo_shared::{
    api::{CreateTagRequest, SetTaskTagsRequest, UpdateTagRequest},
    Tag, WorkspaceRole,
};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::routes::AppState;

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

/// GET /api/v1/workspaces/:id/tags
pub async fn list_tags(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<Tag>>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;

    let tags: Vec<(Uuid, Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT id, workspace_id, name, color
        FROM tags
        WHERE workspace_id = $1
        ORDER BY name
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await?;

    let tags: Vec<Tag> = tags
        .into_iter()
        .map(|(id, workspace_id, name, color)| Tag {
            id,
            workspace_id,
            name,
            color,
        })
        .collect();

    Ok(Json(tags))
}

/// POST /api/v1/workspaces/:id/tags
pub async fn create_tag(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<CreateTagRequest>,
) -> Result<Json<Tag>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    // Only editors and above can create tags
    if role == WorkspaceRole::Reader {
        return Err(AppError::Forbidden);
    }

    let id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO tags (id, workspace_id, name, color)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&req.name)
    .bind(&req.color)
    .execute(&state.db)
    .await?;

    Ok(Json(Tag {
        id,
        workspace_id,
        name: req.name,
        color: req.color,
    }))
}

/// PATCH /api/v1/workspaces/:id/tags/:tag_id
pub async fn update_tag(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, tag_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateTagRequest>,
) -> Result<Json<Tag>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if role == WorkspaceRole::Reader {
        return Err(AppError::Forbidden);
    }

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut param_idx = 3;

    if req.name.is_some() {
        updates.push(format!("name = ${}", param_idx));
        param_idx += 1;
    }
    if req.color.is_some() {
        updates.push(format!("color = ${}", param_idx));
    }

    if updates.is_empty() {
        // No updates, just fetch and return
        let tag: (Uuid, Uuid, String, Option<String>) = sqlx::query_as(
            "SELECT id, workspace_id, name, color FROM tags WHERE id = $1 AND workspace_id = $2",
        )
        .bind(tag_id)
        .bind(workspace_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

        return Ok(Json(Tag {
            id: tag.0,
            workspace_id: tag.1,
            name: tag.2,
            color: tag.3,
        }));
    }

    let query = format!(
        "UPDATE tags SET {} WHERE id = $1 AND workspace_id = $2 RETURNING id, workspace_id, name, color",
        updates.join(", ")
    );

    let mut q = sqlx::query_as::<_, (Uuid, Uuid, String, Option<String>)>(&query)
        .bind(tag_id)
        .bind(workspace_id);

    if let Some(ref name) = req.name {
        q = q.bind(name);
    }
    if let Some(ref color) = req.color {
        q = q.bind(color);
    }

    let tag = q
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(Tag {
        id: tag.0,
        workspace_id: tag.1,
        name: tag.2,
        color: tag.3,
    }))
}

/// DELETE /api/v1/workspaces/:id/tags/:tag_id
pub async fn delete_tag(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, tag_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if role == WorkspaceRole::Reader {
        return Err(AppError::Forbidden);
    }

    let result = sqlx::query("DELETE FROM tags WHERE id = $1 AND workspace_id = $2")
        .bind(tag_id)
        .bind(workspace_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}

/// PUT /api/v1/workspaces/:id/tasks/:task_id/tags
pub async fn set_task_tags(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<SetTaskTagsRequest>,
) -> Result<Json<Vec<Tag>>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if role == WorkspaceRole::Reader {
        return Err(AppError::Forbidden);
    }

    // Verify task exists
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM tasks WHERE id = $1 AND workspace_id = $2")
            .bind(task_id)
            .bind(workspace_id)
            .fetch_optional(&state.db)
            .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }

    // Delete existing task tags
    sqlx::query("DELETE FROM task_tags WHERE task_id = $1")
        .bind(task_id)
        .execute(&state.db)
        .await?;

    // Insert new task tags
    for tag_id in &req.tag_ids {
        sqlx::query("INSERT INTO task_tags (task_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(task_id)
            .bind(tag_id)
            .execute(&state.db)
            .await?;
    }

    // Return the updated tags
    let tags: Vec<(Uuid, Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT t.id, t.workspace_id, t.name, t.color
        FROM tags t
        INNER JOIN task_tags tt ON t.id = tt.tag_id
        WHERE tt.task_id = $1
        ORDER BY t.name
        "#,
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await?;

    let tags: Vec<Tag> = tags
        .into_iter()
        .map(|(id, workspace_id, name, color)| Tag {
            id,
            workspace_id,
            name,
            color,
        })
        .collect();

    Ok(Json(tags))
}

/// GET /api/v1/workspaces/:id/tasks/:task_id/tags
pub async fn get_task_tags(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<Tag>>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;

    let tags: Vec<(Uuid, Uuid, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT t.id, t.workspace_id, t.name, t.color
        FROM tags t
        INNER JOIN task_tags tt ON t.id = tt.tag_id
        WHERE tt.task_id = $1
        ORDER BY t.name
        "#,
    )
    .bind(task_id)
    .fetch_all(&state.db)
    .await?;

    let tags: Vec<Tag> = tags
        .into_iter()
        .map(|(id, workspace_id, name, color)| Tag {
            id,
            workspace_id,
            name,
            color,
        })
        .collect();

    Ok(Json(tags))
}
