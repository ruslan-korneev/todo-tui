use axum::{
    extract::{Path, State},
    http::StatusCode,
    Extension, Json,
};
use chrono::{Duration, Utc};
use todo_shared::{
    api::{
        CreateWorkspaceRequest, InviteMemberRequest, InviteDetails, UpdateMemberRoleRequest,
        UpdateWorkspaceRequest, WorkspaceInvite, WorkspaceMemberWithUser,
    },
    Workspace, WorkspaceRole, WorkspaceSettings, WorkspaceWithRole,
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
        settings: WorkspaceSettings::default(),
        created_at: now,
        updated_at: now,
    }))
}

/// GET /api/v1/workspaces
pub async fn list_workspaces(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<Vec<WorkspaceWithRole>>, AppError> {
    let rows: Vec<(Uuid, String, String, Option<String>, Uuid, serde_json::Value, chrono::DateTime<Utc>, chrono::DateTime<Utc>, WorkspaceRole)> = sqlx::query_as(
        r#"
        SELECT w.id, w.name, w.slug, w.description, w.owner_id, w.settings, w.created_at, w.updated_at, wm.role as "role: WorkspaceRole"
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
        .map(|(id, name, slug, description, owner_id, settings_json, created_at, updated_at, role)| {
            let settings: WorkspaceSettings = serde_json::from_value(settings_json).unwrap_or_default();
            WorkspaceWithRole {
                workspace: Workspace {
                    id,
                    name,
                    slug,
                    description,
                    owner_id,
                    settings,
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
    let row: Option<(Uuid, String, String, Option<String>, Uuid, serde_json::Value, chrono::DateTime<Utc>, chrono::DateTime<Utc>, WorkspaceRole)> = sqlx::query_as(
        r#"
        SELECT w.id, w.name, w.slug, w.description, w.owner_id, w.settings, w.created_at, w.updated_at, wm.role as "role: WorkspaceRole"
        FROM workspaces w
        JOIN workspace_members wm ON wm.workspace_id = w.id
        WHERE w.id = $1 AND wm.user_id = $2
        "#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let (id, name, slug, description, owner_id, settings_json, created_at, updated_at, role) =
        row.ok_or(AppError::NotFound)?;

    let settings: WorkspaceSettings = serde_json::from_value(settings_json).unwrap_or_default();

    Ok(Json(WorkspaceWithRole {
        workspace: Workspace {
            id,
            name,
            slug,
            description,
            owner_id,
            settings,
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
    let settings_json = req.settings.as_ref().map(|s| serde_json::to_value(s).unwrap_or_default());

    // Build dynamic update query
    let row: (Uuid, String, String, Option<String>, Uuid, serde_json::Value, chrono::DateTime<Utc>, chrono::DateTime<Utc>) = sqlx::query_as(
        r#"
        UPDATE workspaces
        SET name = COALESCE($1, name),
            description = COALESCE($2, description),
            settings = COALESCE($3, settings),
            updated_at = $4
        WHERE id = $5
        RETURNING id, name, slug, description, owner_id, settings, created_at, updated_at
        "#,
    )
    .bind(&req.name)
    .bind(&req.description)
    .bind(&settings_json)
    .bind(now)
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await?;

    let settings: WorkspaceSettings = serde_json::from_value(row.5).unwrap_or_default();

    Ok(Json(Workspace {
        id: row.0,
        name: row.1,
        slug: row.2,
        description: row.3,
        owner_id: row.4,
        settings,
        created_at: row.6,
        updated_at: row.7,
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

/// GET /api/v1/workspaces/:id/members
pub async fn list_members(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<WorkspaceMemberWithUser>>, AppError> {
    // Check user has access to workspace
    let access: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    if access.is_none() {
        return Err(AppError::NotFound);
    }

    let rows: Vec<(Uuid, String, String, WorkspaceRole)> = sqlx::query_as(
        r#"
        SELECT u.id, u.display_name, u.email, wm.role as "role: WorkspaceRole"
        FROM workspace_members wm
        JOIN users u ON u.id = wm.user_id
        WHERE wm.workspace_id = $1
        ORDER BY
            CASE wm.role
                WHEN 'owner' THEN 1
                WHEN 'admin' THEN 2
                WHEN 'editor' THEN 3
                WHEN 'reader' THEN 4
            END,
            u.display_name
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await?;

    let members = rows
        .into_iter()
        .map(|(user_id, display_name, email, role)| WorkspaceMemberWithUser {
            user_id,
            display_name,
            email,
            role,
        })
        .collect();

    Ok(Json(members))
}

/// POST /api/v1/workspaces/:id/invites
pub async fn create_invite(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<InviteMemberRequest>,
) -> Result<Json<WorkspaceInvite>, AppError> {
    // Check user has admin permission
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

    // Validate email
    if req.email.trim().is_empty() || !req.email.contains('@') {
        return Err(AppError::Validation("Valid email is required".to_string()));
    }

    // Check if user is already a member
    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT u.id FROM users u
        JOIN workspace_members wm ON wm.user_id = u.id
        WHERE LOWER(u.email) = LOWER($1) AND wm.workspace_id = $2
        "#,
    )
    .bind(&req.email)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::Validation(
            "User is already a member of this workspace".to_string(),
        ));
    }

    // Cannot invite as owner
    if req.role.is_owner() {
        return Err(AppError::Validation("Cannot invite as owner".to_string()));
    }

    let invite_id = Uuid::new_v4();
    let token = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::days(7);

    sqlx::query(
        r#"
        INSERT INTO workspace_invites (id, workspace_id, email, role, token, invited_by, expires_at, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(invite_id)
    .bind(workspace_id)
    .bind(&req.email)
    .bind(&req.role)
    .bind(&token)
    .bind(user.id)
    .bind(expires_at)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok(Json(WorkspaceInvite {
        id: invite_id,
        workspace_id,
        email: req.email,
        role: req.role,
        token,
        expires_at,
        created_at: now,
    }))
}

/// GET /api/v1/invites/:token
pub async fn get_invite(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Json<InviteDetails>, AppError> {
    let row: Option<(String, String, WorkspaceRole, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT w.name, u.display_name, i.role as "role: WorkspaceRole", i.expires_at, i.accepted_at
        FROM workspace_invites i
        JOIN workspaces w ON w.id = i.workspace_id
        JOIN users u ON u.id = i.invited_by
        WHERE i.token = $1
        "#,
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?;

    let (workspace_name, inviter_name, role, expires_at, accepted_at) =
        row.ok_or(AppError::NotFound)?;

    // Check if already accepted
    if accepted_at.is_some() {
        return Err(AppError::Validation("Invite has already been used".to_string()));
    }

    // Check if expired
    if expires_at < Utc::now() {
        return Err(AppError::Validation("Invite has expired".to_string()));
    }

    Ok(Json(InviteDetails {
        workspace_name,
        inviter_name,
        role,
        expires_at,
    }))
}

/// POST /api/v1/invites/:token/accept
pub async fn accept_invite(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(token): Path<String>,
) -> Result<Json<WorkspaceWithRole>, AppError> {
    // Get invite details
    let row: Option<(Uuid, Uuid, WorkspaceRole, chrono::DateTime<Utc>, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT i.id, i.workspace_id, i.role as "role: WorkspaceRole", i.expires_at, i.accepted_at
        FROM workspace_invites i
        WHERE i.token = $1
        "#,
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?;

    let (invite_id, workspace_id, role, expires_at, accepted_at) =
        row.ok_or(AppError::NotFound)?;

    // Check if already accepted
    if accepted_at.is_some() {
        return Err(AppError::Validation("Invite has already been used".to_string()));
    }

    // Check if expired
    if expires_at < Utc::now() {
        return Err(AppError::Validation("Invite has expired".to_string()));
    }

    // Check if user is already a member
    let existing: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::Validation(
            "You are already a member of this workspace".to_string(),
        ));
    }

    let now = Utc::now();

    // Add user as member
    sqlx::query(
        r#"
        INSERT INTO workspace_members (workspace_id, user_id, role, joined_at, invited_by)
        VALUES ($1, $2, $3, $4, (SELECT invited_by FROM workspace_invites WHERE id = $5))
        "#,
    )
    .bind(workspace_id)
    .bind(user.id)
    .bind(&role)
    .bind(now)
    .bind(invite_id)
    .execute(&state.db)
    .await?;

    // Mark invite as accepted
    sqlx::query("UPDATE workspace_invites SET accepted_at = $1 WHERE id = $2")
        .bind(now)
        .bind(invite_id)
        .execute(&state.db)
        .await?;

    // Return workspace with role
    let workspace_row: (Uuid, String, String, Option<String>, Uuid, serde_json::Value, chrono::DateTime<Utc>, chrono::DateTime<Utc>) = sqlx::query_as(
        r#"
        SELECT id, name, slug, description, owner_id, settings, created_at, updated_at
        FROM workspaces WHERE id = $1
        "#,
    )
    .bind(workspace_id)
    .fetch_one(&state.db)
    .await?;

    let settings: WorkspaceSettings = serde_json::from_value(workspace_row.5).unwrap_or_default();

    Ok(Json(WorkspaceWithRole {
        workspace: Workspace {
            id: workspace_row.0,
            name: workspace_row.1,
            slug: workspace_row.2,
            description: workspace_row.3,
            owner_id: workspace_row.4,
            settings,
            created_at: workspace_row.6,
            updated_at: workspace_row.7,
        },
        role,
    }))
}

/// PUT /api/v1/workspaces/:id/members/:user_id
pub async fn update_member_role(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, member_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateMemberRoleRequest>,
) -> Result<Json<WorkspaceMemberWithUser>, AppError> {
    // Check user has admin permission
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

    // Get target member's current role
    let target_role: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(member_id)
    .fetch_optional(&state.db)
    .await?;

    let (target_role,) = target_role.ok_or(AppError::NotFound)?;

    // Cannot change owner's role
    if target_role.is_owner() {
        return Err(AppError::Validation("Cannot change owner's role".to_string()));
    }

    // Cannot promote to owner
    if req.role.is_owner() {
        return Err(AppError::Validation("Cannot promote to owner".to_string()));
    }

    // Cannot change own role
    if member_id == user.id {
        return Err(AppError::Validation("Cannot change your own role".to_string()));
    }

    // Update role
    sqlx::query("UPDATE workspace_members SET role = $1 WHERE workspace_id = $2 AND user_id = $3")
        .bind(&req.role)
        .bind(workspace_id)
        .bind(member_id)
        .execute(&state.db)
        .await?;

    // Return updated member
    let row: (Uuid, String, String, WorkspaceRole) = sqlx::query_as(
        r#"
        SELECT u.id, u.display_name, u.email, wm.role as "role: WorkspaceRole"
        FROM workspace_members wm
        JOIN users u ON u.id = wm.user_id
        WHERE wm.workspace_id = $1 AND wm.user_id = $2
        "#,
    )
    .bind(workspace_id)
    .bind(member_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(WorkspaceMemberWithUser {
        user_id: row.0,
        display_name: row.1,
        email: row.2,
        role: row.3,
    }))
}

/// DELETE /api/v1/workspaces/:id/members/:user_id
pub async fn remove_member(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, member_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, AppError> {
    // Check user has admin permission
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

    // Get target member's role
    let target_role: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"SELECT role as "role: WorkspaceRole" FROM workspace_members WHERE workspace_id = $1 AND user_id = $2"#,
    )
    .bind(workspace_id)
    .bind(member_id)
    .fetch_optional(&state.db)
    .await?;

    let (target_role,) = target_role.ok_or(AppError::NotFound)?;

    // Cannot remove owner
    if target_role.is_owner() {
        return Err(AppError::Validation("Cannot remove workspace owner".to_string()));
    }

    // Cannot remove self
    if member_id == user.id {
        return Err(AppError::Validation(
            "Cannot remove yourself. Use leave workspace instead.".to_string(),
        ));
    }

    // Remove member
    sqlx::query("DELETE FROM workspace_members WHERE workspace_id = $1 AND user_id = $2")
        .bind(workspace_id)
        .bind(member_id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}
