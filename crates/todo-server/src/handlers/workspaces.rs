use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::Utc;
use todo_shared::{
    api::{CreateWorkspaceRequest, UpdateWorkspaceRequest},
    Workspace, WorkspaceRole, WorkspaceWithRole,
};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::routes::AppState;

/// Generate URL-friendly slug from name
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// POST /api/v1/workspaces
pub async fn create_workspace(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Json(req): Json<CreateWorkspaceRequest>,
) -> Result<Json<Workspace>, AppError> {
    if req.name.trim().is_empty() {
        return Err(AppError::Validation("Workspace name is required".to_string()));
    }

    let workspace_id = Uuid::new_v4();
    let base_slug = slugify(&req.name);

    // Ensure unique slug by appending random suffix if needed
    let slug = format!("{}-{}", base_slug, &workspace_id.to_string()[..8]);

    let now = Utc::now();

    // Create workspace
    sqlx::query(
        r#"
        INSERT INTO workspaces (id, name, slug, description, owner_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(workspace_id)
    .bind(&req.name)
    .bind(&slug)
    .bind(&req.description)
    .bind(user.id)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    // Add owner as member
    sqlx::query(
        r#"
        INSERT INTO workspace_members (workspace_id, user_id, role, joined_at)
        VALUES ($1, $2, 'owner', $3)
        "#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .bind(now)
    .execute(&state.db)
    .await?;

    // Create default statuses
    let default_statuses = [
        ("To Do", "todo", "#6B7280", false, 0),
        ("In Progress", "in-progress", "#3B82F6", false, 1),
        ("Done", "done", "#10B981", true, 2),
    ];

    for (name, status_slug, color, is_done, position) in default_statuses {
        sqlx::query(
            r#"
            INSERT INTO task_statuses (id, workspace_id, name, slug, color, is_done, position, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(workspace_id)
        .bind(name)
        .bind(status_slug)
        .bind(color)
        .bind(is_done)
        .bind(position)
        .bind(now)
        .execute(&state.db)
        .await?;
    }

    Ok(Json(Workspace {
        id: workspace_id,
        name: req.name,
        slug,
        description: req.description,
        owner_id: user.id,
        created_at: now,
        updated_at: now,
    }))
}

/// GET /api/v1/workspaces
pub async fn list_workspaces(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<WorkspaceWithRole>>, AppError> {
    let rows: Vec<(Uuid, String, String, Option<String>, Uuid, chrono::DateTime<Utc>, chrono::DateTime<Utc>, WorkspaceRole)> = sqlx::query_as(
        r#"
        SELECT w.id, w.name, w.slug, w.description, w.owner_id, w.created_at, w.updated_at, wm.role as "role: WorkspaceRole"
        FROM workspaces w
        JOIN workspace_members wm ON wm.workspace_id = w.id
        WHERE wm.user_id = $1
        ORDER BY w.created_at DESC
        "#,
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let workspaces = rows
        .into_iter()
        .map(|(id, name, slug, description, owner_id, created_at, updated_at, role)| {
            WorkspaceWithRole {
                workspace: Workspace {
                    id,
                    name,
                    slug,
                    description,
                    owner_id,
                    created_at,
                    updated_at,
                },
                role,
            }
        })
        .collect();

    Ok(Json(workspaces))
}

/// GET /api/v1/workspaces/:id
pub async fn get_workspace(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<WorkspaceWithRole>, AppError> {
    let row: Option<(Uuid, String, String, Option<String>, Uuid, chrono::DateTime<Utc>, chrono::DateTime<Utc>, WorkspaceRole)> = sqlx::query_as(
        r#"
        SELECT w.id, w.name, w.slug, w.description, w.owner_id, w.created_at, w.updated_at, wm.role as "role: WorkspaceRole"
        FROM workspaces w
        JOIN workspace_members wm ON wm.workspace_id = w.id
        WHERE w.id = $1 AND wm.user_id = $2
        "#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let (id, name, slug, description, owner_id, created_at, updated_at, role) =
        row.ok_or(AppError::NotFound)?;

    Ok(Json(WorkspaceWithRole {
        workspace: Workspace {
            id,
            name,
            slug,
            description,
            owner_id,
            created_at,
            updated_at,
        },
        role,
    }))
}

/// PATCH /api/v1/workspaces/:id
pub async fn update_workspace(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<UpdateWorkspaceRequest>,
) -> Result<Json<Workspace>, AppError> {
    // Check membership and role
    let role: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let (role,) = role.ok_or(AppError::NotFound)?;

    if !role.can_admin() {
        return Err(AppError::Forbidden);
    }

    let now = Utc::now();

    // Build dynamic update query
    let row: (Uuid, String, String, Option<String>, Uuid, chrono::DateTime<Utc>, chrono::DateTime<Utc>) = sqlx::query_as(
        r#"
        UPDATE workspaces
        SET name = COALESCE($1, name),
            description = COALESCE($2, description),
            updated_at = $3
        WHERE id = $4
        RETURNING id, name, slug, description, owner_id, created_at, updated_at
        "#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(now)
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(Workspace {
        id: row.0,
        name: row.1,
        slug: row.2,
        description: row.3,
        owner_id: row.4,
        created_at: row.5,
        updated_at: row.6,
    }))
}

/// DELETE /api/v1/workspaces/:id
pub async fn delete_workspace(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
) -> Result<(), AppError> {
    // Check if user is owner
    let role: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let (role,) = role.ok_or(AppError::NotFound)?;

    if !role.is_owner() {
        return Err(AppError::Forbidden);
    }

    // Delete workspace (cascades to members, statuses, tasks, etc.)
    sqlx::query("DELETE FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .execute(&state.db)
        .await?;

    Ok(())
}
