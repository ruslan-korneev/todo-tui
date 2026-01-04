use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use todo_shared::{
    api::{CreateTaskRequest, MoveTaskRequest, UpdateTaskRequest},
    Priority, Task, WorkspaceRole,
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

/// Helper to verify status belongs to workspace
async fn verify_status(
    state: &AppState,
    status_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    let exists: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM task_statuses WHERE id = $1 AND workspace_id = $2",
    )
    .bind(status_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await?;

    if exists.is_none() {
        return Err(AppError::Validation(
            "Invalid status_id for this workspace".to_string(),
        ));
    }
    Ok(())
}

type TaskRow = (
    Uuid,                          // id
    Uuid,                          // workspace_id
    Uuid,                          // status_id
    String,                        // title
    Option<String>,                // description
    Option<Priority>,              // priority
    Option<NaiveDate>,             // due_date
    Option<i32>,                   // time_estimate_minutes
    i32,                           // position
    Uuid,                          // created_by
    Option<Uuid>,                  // assigned_to
    chrono::DateTime<Utc>,         // created_at
    chrono::DateTime<Utc>,         // updated_at
    Option<chrono::DateTime<Utc>>, // completed_at
);

fn row_to_task(row: TaskRow) -> Task {
    Task {
        id: row.0,
        workspace_id: row.1,
        status_id: row.2,
        title: row.3,
        description: row.4,
        priority: row.5,
        due_date: row.6,
        time_estimate_minutes: row.7,
        position: row.8,
        created_by: row.9,
        assigned_to: row.10,
        created_at: row.11,
        updated_at: row.12,
        completed_at: row.13,
    }
}

#[derive(Debug, Deserialize)]
pub struct TaskListQuery {
    pub status_id: Option<Uuid>,
    pub priority: Option<Priority>,
    pub assigned_to: Option<Uuid>,
    pub due_before: Option<NaiveDate>,
    pub due_after: Option<NaiveDate>,
    pub q: Option<String>,
    pub order_by: Option<String>,
    pub order: Option<String>,
    pub page: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Serialize)]
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
}

/// GET /api/v1/workspaces/:id/tasks
pub async fn list_tasks(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Query(params): Query<TaskListQuery>,
) -> Result<Json<TaskListResponse>, AppError> {
    // Verify membership
    check_membership(&state, workspace_id, user.id).await?;

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(50).min(100);
    let offset = (page - 1) * limit;

    // Build dynamic query
    let mut conditions = vec!["workspace_id = $1".to_string()];
    let mut param_idx = 2;

    if params.status_id.is_some() {
        conditions.push(format!("status_id = ${}", param_idx));
        param_idx += 1;
    }
    if params.priority.is_some() {
        conditions.push(format!("priority = ${}", param_idx));
        param_idx += 1;
    }
    if params.assigned_to.is_some() {
        conditions.push(format!("assigned_to = ${}", param_idx));
        param_idx += 1;
    }
    if params.due_before.is_some() {
        conditions.push(format!("due_date <= ${}", param_idx));
        param_idx += 1;
    }
    if params.due_after.is_some() {
        conditions.push(format!("due_date >= ${}", param_idx));
        param_idx += 1;
    }
    if params.q.is_some() {
        conditions.push(format!(
            "(title ILIKE ${} OR description ILIKE ${})",
            param_idx,
            param_idx + 1
        ));
        param_idx += 2;
    }

    let where_clause = conditions.join(" AND ");

    let order_by = match params.order_by.as_deref() {
        Some("title") => "title",
        Some("priority") => "priority",
        Some("due_date") => "due_date",
        Some("created_at") => "created_at",
        Some("updated_at") => "updated_at",
        _ => "position",
    };

    let order = match params.order.as_deref() {
        Some("desc") | Some("DESC") => "DESC",
        _ => "ASC",
    };

    // Count total
    let count_query = format!("SELECT COUNT(*) FROM tasks WHERE {}", where_clause);
    let mut count_builder = sqlx::query_as::<_, (i64,)>(&count_query).bind(workspace_id);

    if let Some(ref status_id) = params.status_id {
        count_builder = count_builder.bind(status_id);
    }
    if let Some(ref priority) = params.priority {
        count_builder = count_builder.bind(priority);
    }
    if let Some(ref assigned_to) = params.assigned_to {
        count_builder = count_builder.bind(assigned_to);
    }
    if let Some(ref due_before) = params.due_before {
        count_builder = count_builder.bind(due_before);
    }
    if let Some(ref due_after) = params.due_after {
        count_builder = count_builder.bind(due_after);
    }
    if let Some(ref q) = params.q {
        let pattern = format!("%{}%", q);
        count_builder = count_builder.bind(pattern.clone()).bind(pattern);
    }

    let (total,): (i64,) = count_builder.fetch_one(&state.db).await?;

    // Fetch tasks
    let select_query = format!(
        r#"
        SELECT id, workspace_id, status_id, title, description,
               priority as "priority: Priority", due_date, time_estimate_minutes,
               position, created_by, assigned_to, created_at, updated_at, completed_at
        FROM tasks
        WHERE {}
        ORDER BY {} {}
        LIMIT ${} OFFSET ${}
        "#,
        where_clause, order_by, order, param_idx, param_idx + 1
    );

    let mut select_builder = sqlx::query_as::<_, TaskRow>(&select_query).bind(workspace_id);

    if let Some(ref status_id) = params.status_id {
        select_builder = select_builder.bind(status_id);
    }
    if let Some(ref priority) = params.priority {
        select_builder = select_builder.bind(priority);
    }
    if let Some(ref assigned_to) = params.assigned_to {
        select_builder = select_builder.bind(assigned_to);
    }
    if let Some(ref due_before) = params.due_before {
        select_builder = select_builder.bind(due_before);
    }
    if let Some(ref due_after) = params.due_after {
        select_builder = select_builder.bind(due_after);
    }
    if let Some(ref q) = params.q {
        let pattern = format!("%{}%", q);
        select_builder = select_builder.bind(pattern.clone()).bind(pattern);
    }

    select_builder = select_builder.bind(limit as i64).bind(offset as i64);

    let rows = select_builder.fetch_all(&state.db).await?;
    let tasks = rows.into_iter().map(row_to_task).collect();

    Ok(Json(TaskListResponse {
        tasks,
        total,
        page,
        limit,
    }))
}

/// POST /api/v1/workspaces/:id/tasks
pub async fn create_task(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<Json<Task>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    if req.title.trim().is_empty() {
        return Err(AppError::Validation("Task title is required".to_string()));
    }

    // Verify status belongs to workspace
    verify_status(&state, req.status_id, workspace_id).await?;

    let id = Uuid::new_v4();
    let now = Utc::now();

    // Get max position in status
    let (max_pos,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(MAX(position), -1) FROM tasks WHERE status_id = $1",
    )
    .bind(req.status_id)
    .fetch_one(&state.db)
    .await?;

    let position = max_pos + 1;

    sqlx::query(
        r#"
        INSERT INTO tasks (id, workspace_id, status_id, title, description, priority,
                          due_date, time_estimate_minutes, position, created_by,
                          assigned_to, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        "#,
    )
    .bind(id)
    .bind(workspace_id)
    .bind(req.status_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.priority)
    .bind(req.due_date)
    .bind(req.time_estimate_minutes)
    .bind(position)
    .bind(user.id)
    .bind(req.assigned_to)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok(Json(Task {
        id,
        workspace_id,
        status_id: req.status_id,
        title: req.title,
        description: req.description,
        priority: req.priority,
        due_date: req.due_date,
        time_estimate_minutes: req.time_estimate_minutes,
        position,
        created_by: user.id,
        assigned_to: req.assigned_to,
        created_at: now,
        updated_at: now,
        completed_at: None,
    }))
}

/// GET /api/v1/workspaces/:id/tasks/:task_id
pub async fn get_task(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Task>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;

    let row: TaskRow = sqlx::query_as(
        r#"
        SELECT id, workspace_id, status_id, title, description,
               priority as "priority: Priority", due_date, time_estimate_minutes,
               position, created_by, assigned_to, created_at, updated_at, completed_at
        FROM tasks
        WHERE id = $1 AND workspace_id = $2
        "#,
    )
    .bind(task_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(row_to_task(row)))
}

/// PATCH /api/v1/workspaces/:id/tasks/:task_id
pub async fn update_task(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<Task>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    verify_task(&state, task_id, workspace_id).await?;

    // If status_id is being changed, verify the new status
    if let Some(ref status_id) = req.status_id {
        verify_status(&state, *status_id, workspace_id).await?;
    }

    let now = Utc::now();

    // Check if moving to a "done" status
    let completed_at = if let Some(ref status_id) = req.status_id {
        let (is_done,): (bool,) = sqlx::query_as(
            "SELECT is_done FROM task_statuses WHERE id = $1",
        )
        .bind(status_id)
        .fetch_one(&state.db)
        .await?;

        if is_done {
            Some(now)
        } else {
            None
        }
    } else {
        // Keep existing completed_at
        let (existing,): (Option<chrono::DateTime<Utc>>,) = sqlx::query_as(
            "SELECT completed_at FROM tasks WHERE id = $1",
        )
        .bind(task_id)
        .fetch_one(&state.db)
        .await?;
        existing
    };

    let row: TaskRow = sqlx::query_as(
        r#"
        UPDATE tasks
        SET title = COALESCE($1, title),
            status_id = COALESCE($2, status_id),
            description = COALESCE($3, description),
            priority = COALESCE($4, priority),
            due_date = COALESCE($5, due_date),
            time_estimate_minutes = COALESCE($6, time_estimate_minutes),
            assigned_to = COALESCE($7, assigned_to),
            updated_at = $8,
            completed_at = $9
        WHERE id = $10
        RETURNING id, workspace_id, status_id, title, description,
                  priority as "priority: Priority", due_date, time_estimate_minutes,
                  position, created_by, assigned_to, created_at, updated_at, completed_at
        "#,
    )
    .bind(&req.title)
    .bind(req.status_id)
    .bind(&req.description)
    .bind(&req.priority)
    .bind(req.due_date)
    .bind(req.time_estimate_minutes)
    .bind(req.assigned_to)
    .bind(now)
    .bind(completed_at)
    .bind(task_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row_to_task(row)))
}

/// DELETE /api/v1/workspaces/:id/tasks/:task_id
pub async fn delete_task(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    let result = sqlx::query("DELETE FROM tasks WHERE id = $1 AND workspace_id = $2")
        .bind(task_id)
        .bind(workspace_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}

/// POST /api/v1/workspaces/:id/tasks/:task_id/move
pub async fn move_task(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, task_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MoveTaskRequest>,
) -> Result<Json<Task>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    verify_task(&state, task_id, workspace_id).await?;
    verify_status(&state, req.status_id, workspace_id).await?;

    let mut tx = state.db.begin().await?;

    // Check if the target status is a "done" status
    let (is_done,): (bool,) = sqlx::query_as(
        "SELECT is_done FROM task_statuses WHERE id = $1",
    )
    .bind(req.status_id)
    .fetch_one(&mut *tx)
    .await?;

    let now = Utc::now();
    let completed_at = if is_done { Some(now) } else { None };

    // Calculate new position
    let new_position = if let Some(pos) = req.position {
        // Shift tasks at and after the target position
        sqlx::query(
            "UPDATE tasks SET position = position + 1 WHERE status_id = $1 AND position >= $2",
        )
        .bind(req.status_id)
        .bind(pos)
        .execute(&mut *tx)
        .await?;
        pos
    } else {
        // Append to end
        let (max_pos,): (i32,) = sqlx::query_as(
            "SELECT COALESCE(MAX(position), -1) FROM tasks WHERE status_id = $1",
        )
        .bind(req.status_id)
        .fetch_one(&mut *tx)
        .await?;
        max_pos + 1
    };

    let row: TaskRow = sqlx::query_as(
        r#"
        UPDATE tasks
        SET status_id = $1, position = $2, updated_at = $3, completed_at = $4
        WHERE id = $5
        RETURNING id, workspace_id, status_id, title, description,
                  priority as "priority: Priority", due_date, time_estimate_minutes,
                  position, created_by, assigned_to, created_at, updated_at, completed_at
        "#,
    )
    .bind(req.status_id)
    .bind(new_position)
    .bind(now)
    .bind(completed_at)
    .bind(task_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(row_to_task(row)))
}
