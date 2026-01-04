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

/// Search result row from database with highlight fields
#[derive(sqlx::FromRow)]
struct SearchTaskRow {
    id: Uuid,
    workspace_id: Uuid,
    status_id: Uuid,
    title: String,
    description: Option<String>,
    #[sqlx(rename = "priority")]
    priority: Option<Priority>,
    due_date: Option<NaiveDate>,
    time_estimate_minutes: Option<i32>,
    position: i32,
    created_by: Uuid,
    assigned_to: Option<Uuid>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    rank: f32,
    title_highlight: Option<String>,
    desc_highlight: Option<String>,
}

fn row_to_search_result(row: SearchTaskRow) -> SearchResultItem {
    SearchResultItem::Task(SearchTaskResult {
        task: Task {
            id: row.id,
            workspace_id: row.workspace_id,
            status_id: row.status_id,
            title: row.title,
            description: row.description,
            priority: row.priority,
            due_date: row.due_date,
            time_estimate_minutes: row.time_estimate_minutes,
            position: row.position,
            created_by: row.created_by,
            assigned_to: row.assigned_to,
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
            tags: Vec::new(),
        },
        rank: row.rank,
        title_highlights: row.title_highlight,
        description_highlights: row.desc_highlight,
    })
}

/// Generate highlight markers for fuzzy search matches
fn highlight_fuzzy_matches(text: &str, query: &str) -> String {
    if query.is_empty() || text.is_empty() {
        return text.to_string();
    }

    // Case-insensitive search for query substring
    let text_lower = text.to_lowercase();
    let query_lower = query.to_lowercase();

    let mut result = String::with_capacity(text.len() + query.len() * 4);
    let mut last_end = 0;

    // Find all occurrences of the query in the text
    for (start, _) in text_lower.match_indices(&query_lower) {
        // Add text before this match
        result.push_str(&text[last_end..start]);
        // Add highlighted match (using original case)
        result.push_str("<<");
        result.push_str(&text[start..start + query.len()]);
        result.push_str(">>");
        last_end = start + query.len();
    }

    // Add remaining text
    result.push_str(&text[last_end..]);
    result
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
                   )::real as rank,
                   NULL::text as title_highlight,
                   NULL::text as desc_highlight
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

        // Apply fuzzy highlighting in Rust (PostgreSQL doesn't have built-in trigram highlighting)
        let rows: Vec<SearchTaskRow> = rows
            .into_iter()
            .map(|mut row| {
                row.title_highlight = Some(highlight_fuzzy_matches(&row.title, query));
                row.desc_highlight = row
                    .description
                    .as_ref()
                    .map(|d| highlight_fuzzy_matches(d, query));
                row
            })
            .collect();

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
                   )::real as rank,
                   ts_headline('english', t.title, plainto_tsquery('english', $2),
                              'StartSel=<<, StopSel=>>') as title_highlight,
                   ts_headline('english', COALESCE(t.description, ''), plainto_tsquery('english', $2),
                              'StartSel=<<, StopSel=>>, MaxWords=30, MinWords=10') as desc_highlight
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
