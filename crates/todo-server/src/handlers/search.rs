use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use todo_shared::{
    api::{
        SearchDocumentResult, SearchParams, SearchResponse, SearchResultItem, SearchTaskResult,
        SearchType,
    },
    Document, Priority, Task, WorkspaceRole,
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

/// Document search result row from database with highlight fields
#[derive(sqlx::FromRow)]
struct SearchDocumentRow {
    id: Uuid,
    workspace_id: Uuid,
    path: String,
    parent_id: Option<Uuid>,
    title: String,
    slug: String,
    content: Option<String>,
    created_by: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    rank: f32,
    title_highlight: Option<String>,
    content_highlight: Option<String>,
}

fn row_to_document_result(row: SearchDocumentRow) -> SearchResultItem {
    SearchResultItem::Document(SearchDocumentResult {
        document: Document {
            id: row.id,
            workspace_id: row.workspace_id,
            path: row.path,
            parent_id: row.parent_id,
            title: row.title,
            slug: row.slug,
            content: row.content,
            created_by: row.created_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        },
        rank: row.rank,
        title_highlights: row.title_highlight,
        content_highlights: row.content_highlight,
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
    let search_type = params.search_type.unwrap_or_default();

    let search_tasks = matches!(search_type, SearchType::All | SearchType::Tasks);
    let search_docs = matches!(search_type, SearchType::All | SearchType::Documents);

    let mut all_results: Vec<SearchResultItem> = Vec::new();
    let mut total: i64 = 0;

    // Search tasks using trigrams (multilingual)
    if search_tasks {
        let (task_total, task_results) =
            search_tasks_impl(&state, workspace_id, query, limit, offset).await?;
        total += task_total;
        all_results.extend(task_results.into_iter().map(row_to_search_result));
    }

    // Search documents using trigrams (multilingual)
    if search_docs {
        let (doc_total, doc_results) =
            search_documents_impl(&state, workspace_id, query, limit, offset).await?;
        total += doc_total;
        all_results.extend(doc_results.into_iter().map(row_to_document_result));
    }

    // Sort by rank (descending) if searching both types
    if search_tasks && search_docs {
        all_results.sort_by(|a, b| {
            let rank_a = match a {
                SearchResultItem::Task(t) => t.rank,
                SearchResultItem::Document(d) => d.rank,
            };
            let rank_b = match b {
                SearchResultItem::Task(t) => t.rank,
                SearchResultItem::Document(d) => d.rank,
            };
            rank_b
                .partial_cmp(&rank_a)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Limit results when merging
        all_results.truncate(limit as usize);
    }

    Ok(Json(SearchResponse {
        results: all_results,
        total,
        page,
        limit,
        query: query.to_string(),
    }))
}

/// Search tasks using trigrams (pg_trgm) - works with any language
/// Uses word_similarity for better matching of words within longer text
async fn search_tasks_impl(
    state: &AppState,
    workspace_id: Uuid,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<(i64, Vec<SearchTaskRow>), AppError> {
    // Count total matches using word_similarity (finds query as word in text)
    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM tasks t
        WHERE t.workspace_id = $1
          AND ($2 <% t.title OR $2 <% COALESCE(t.description, ''))
        "#,
    )
    .bind(workspace_id)
    .bind(query)
    .fetch_one(&state.db)
    .await?;

    // Get results with word_similarity ranking
    let rows: Vec<SearchTaskRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.workspace_id, t.status_id, t.title, t.description,
               t.priority as "priority: Priority", t.due_date, t.time_estimate_minutes,
               t.position, t.created_by, t.assigned_to, t.created_at, t.updated_at, t.completed_at,
               GREATEST(
                   word_similarity($2, t.title),
                   COALESCE(word_similarity($2, t.description), 0)
               )::real as rank,
               NULL::text as title_highlight,
               NULL::text as desc_highlight
        FROM tasks t
        WHERE t.workspace_id = $1
          AND ($2 <% t.title OR $2 <% COALESCE(t.description, ''))
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

    // Apply highlighting in Rust (PostgreSQL doesn't have built-in trigram highlighting)
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

    Ok((total, rows))
}

/// Search documents using trigrams (pg_trgm) - works with any language
/// Uses word_similarity for better matching of words within longer text
async fn search_documents_impl(
    state: &AppState,
    workspace_id: Uuid,
    query: &str,
    limit: u32,
    offset: u32,
) -> Result<(i64, Vec<SearchDocumentRow>), AppError> {
    // Count total matches using word_similarity (finds query as word in text)
    let (total,): (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*)
        FROM documents d
        WHERE d.workspace_id = $1
          AND ($2 <% d.title OR $2 <% COALESCE(d.content, ''))
        "#,
    )
    .bind(workspace_id)
    .bind(query)
    .fetch_one(&state.db)
    .await?;

    // Get results with word_similarity ranking
    let rows: Vec<SearchDocumentRow> = sqlx::query_as(
        r#"
        SELECT d.id, d.workspace_id, d.path::text, d.parent_id, d.title, d.slug,
               d.content, d.created_by, d.created_at, d.updated_at,
               GREATEST(
                   word_similarity($2, d.title),
                   COALESCE(word_similarity($2, d.content), 0)
               )::real as rank,
               NULL::text as title_highlight,
               NULL::text as content_highlight
        FROM documents d
        WHERE d.workspace_id = $1
          AND ($2 <% d.title OR $2 <% COALESCE(d.content, ''))
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

    // Apply highlighting in Rust (PostgreSQL doesn't have built-in trigram highlighting)
    let rows: Vec<SearchDocumentRow> = rows
        .into_iter()
        .map(|mut row| {
            row.title_highlight = Some(highlight_fuzzy_matches(&row.title, query));
            row.content_highlight = row
                .content
                .as_ref()
                .map(|c| highlight_fuzzy_matches(c, query));
            row
        })
        .collect();

    Ok((total, rows))
}
