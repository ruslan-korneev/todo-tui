use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use todo_shared::{
    api::{SearchParams, SearchResponse, SearchResultItem, SearchTaskResult},
    Priority, Task, WorkspaceRole,
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

// 15 elements (within SQLx tuple limit of 16)
type SearchTaskRow = (
    Uuid,                    // id
    Uuid,                    // workspace_id
    Uuid,                    // status_id
    String,                  // title
    Option<String>,          // description
    Option<Priority>,        // priority
    Option<NaiveDate>,       // due_date
    Option<i32>,             // time_estimate_minutes
    i32,                     // position
    Uuid,                    // created_by
    Option<Uuid>,            // assigned_to
    DateTime<Utc>,           // created_at
    DateTime<Utc>,           // updated_at
    Option<DateTime<Utc>>,   // completed_at
    f32,                     // rank
);

fn row_to_search_result(row: SearchTaskRow) -> SearchResultItem {
    SearchResultItem::Task(SearchTaskResult {
        task: Task {
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
        },
        rank: row.14,
        title_highlights: None,
        description_highlights: None,
    })
}

/// GET /api/v1/workspaces/:id/search
pub async fn search(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Query(params): Query<SearchParams>,
) -> Result<Json<SearchResponse>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;

    let query = params.q.trim();
    if query.is_empty() {
        return Ok(Json(SearchResponse {
            results: vec![],
            total: 0,
            page: 1,
            limit: 20,
            query: String::new(),
        }));
    }

    let page = params.page.unwrap_or(1).max(1);
    let limit = params.limit.unwrap_or(20).min(100);
    let offset = (page - 1) * limit;
    let use_fuzzy = params.fuzzy.unwrap_or(false);

    let (total, results) = if use_fuzzy {
        // Trigram fuzzy search
        let (total,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM tasks t
            WHERE t.workspace_id = $1
              AND (t.title % $2 OR t.description % $2)
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .fetch_one(&state.db)
        .await?;

        let rows: Vec<SearchTaskRow> = sqlx::query_as(
            r#"
            SELECT t.id, t.workspace_id, t.status_id, t.title, t.description,
                   t.priority as "priority: Priority", t.due_date, t.time_estimate_minutes,
                   t.position, t.created_by, t.assigned_to, t.created_at, t.updated_at, t.completed_at,
                   GREATEST(
                       similarity(t.title, $2),
                       COALESCE(similarity(t.description, $2), 0)
                   )::real as rank
            FROM tasks t
            WHERE t.workspace_id = $1
              AND (t.title % $2 OR t.description % $2)
            ORDER BY rank DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&state.db)
        .await?;

        (total, rows)
    } else {
        // Full-text search
        let (total,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM tasks t
            WHERE t.workspace_id = $1
              AND to_tsvector('english', COALESCE(t.title, '') || ' ' || COALESCE(t.description, ''))
                  @@ plainto_tsquery('english', $2)
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .fetch_one(&state.db)
        .await?;

        let rows: Vec<SearchTaskRow> = sqlx::query_as(
            r#"
            SELECT t.id, t.workspace_id, t.status_id, t.title, t.description,
                   t.priority as "priority: Priority", t.due_date, t.time_estimate_minutes,
                   t.position, t.created_by, t.assigned_to, t.created_at, t.updated_at, t.completed_at,
                   ts_rank(
                       to_tsvector('english', COALESCE(t.title, '') || ' ' || COALESCE(t.description, '')),
                       plainto_tsquery('english', $2)
                   )::real as rank
            FROM tasks t
            WHERE t.workspace_id = $1
              AND to_tsvector('english', COALESCE(t.title, '') || ' ' || COALESCE(t.description, ''))
                  @@ plainto_tsquery('english', $2)
            ORDER BY rank DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(workspace_id)
        .bind(query)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&state.db)
        .await?;

        (total, rows)
    };

    let results = results.into_iter().map(row_to_search_result).collect();

    Ok(Json(SearchResponse {
        results,
        total,
        page,
        limit,
        query: query.to_string(),
    }))
}
