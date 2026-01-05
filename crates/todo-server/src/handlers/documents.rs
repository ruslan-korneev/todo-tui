use axum::{
    extract::{Path, State},
    Extension, Json,
};
use chrono::{DateTime, Utc};
use todo_shared::{
    api::{CreateDocumentRequest, MoveDocumentRequest, UpdateDocumentRequest},
    Document, WorkspaceRole,
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

/// Helper to verify document belongs to workspace
async fn verify_document(
    state: &AppState,
    doc_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), AppError> {
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM documents WHERE id = $1 AND workspace_id = $2")
            .bind(doc_id)
            .bind(workspace_id)
            .fetch_optional(&state.db)
            .await?;

    if exists.is_none() {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Generate URL-safe slug from title
fn generate_slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

type DocumentRow = (
    Uuid,                // id
    Uuid,                // workspace_id
    String,              // path
    Option<Uuid>,        // parent_id
    String,              // title
    String,              // slug
    Option<String>,      // content
    Uuid,                // created_by
    DateTime<Utc>,       // created_at
    DateTime<Utc>,       // updated_at
);

fn row_to_document(row: DocumentRow) -> Document {
    Document {
        id: row.0,
        workspace_id: row.1,
        path: row.2,
        parent_id: row.3,
        title: row.4,
        slug: row.5,
        content: row.6,
        created_by: row.7,
        created_at: row.8,
        updated_at: row.9,
    }
}

/// GET /api/v1/workspaces/:id/documents
pub async fn list_documents(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
) -> Result<Json<Vec<Document>>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;

    let rows: Vec<DocumentRow> = sqlx::query_as(
        r#"
        SELECT id, workspace_id, path::text, parent_id, title, slug, content,
               created_by, created_at, updated_at
        FROM documents
        WHERE workspace_id = $1
        ORDER BY path
        "#,
    )
    .bind(workspace_id)
    .fetch_all(&state.db)
    .await?;

    let documents: Vec<Document> = rows.into_iter().map(row_to_document).collect();

    Ok(Json(documents))
}

/// POST /api/v1/workspaces/:id/documents
pub async fn create_document(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path(workspace_id): Path<Uuid>,
    Json(req): Json<CreateDocumentRequest>,
) -> Result<Json<Document>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    if req.title.trim().is_empty() {
        return Err(AppError::Validation("Document title is required".to_string()));
    }

    let id = Uuid::new_v4();
    let now = Utc::now();
    let slug = generate_slug(&req.title);

    // Generate path based on parent
    let path = if let Some(parent_id) = req.parent_id {
        // Verify parent belongs to workspace
        let parent: Option<(String,)> = sqlx::query_as(
            "SELECT path::text FROM documents WHERE id = $1 AND workspace_id = $2",
        )
        .bind(parent_id)
        .bind(workspace_id)
        .fetch_optional(&state.db)
        .await?;

        match parent {
            Some((parent_path,)) => format!("{}.{}", parent_path, slug),
            None => {
                return Err(AppError::Validation(
                    "Parent document not found in this workspace".to_string(),
                ))
            }
        }
    } else {
        slug.clone()
    };

    // Check for duplicate path
    let existing: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM documents WHERE workspace_id = $1 AND path = $2::ltree")
            .bind(workspace_id)
            .bind(&path)
            .fetch_optional(&state.db)
            .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(
            "A document with this path already exists".to_string(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO documents (id, workspace_id, path, parent_id, title, slug, content,
                               created_by, created_at, updated_at)
        VALUES ($1, $2, $3::ltree, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&path)
    .bind(req.parent_id)
    .bind(&req.title)
    .bind(&slug)
    .bind(&req.content)
    .bind(user.id)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok(Json(Document {
        id,
        workspace_id,
        path,
        parent_id: req.parent_id,
        title: req.title,
        slug,
        content: req.content,
        created_by: user.id,
        created_at: now,
        updated_at: now,
    }))
}

/// GET /api/v1/workspaces/:id/documents/:doc_id
pub async fn get_document(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, doc_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Document>, AppError> {
    check_membership(&state, workspace_id, user.id).await?;

    let row: DocumentRow = sqlx::query_as(
        r#"
        SELECT id, workspace_id, path::text, parent_id, title, slug, content,
               created_by, created_at, updated_at
        FROM documents
        WHERE id = $1 AND workspace_id = $2
        "#,
    )
    .bind(doc_id)
    .bind(workspace_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(Json(row_to_document(row)))
}

/// PATCH /api/v1/workspaces/:id/documents/:doc_id
pub async fn update_document(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, doc_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<UpdateDocumentRequest>,
) -> Result<Json<Document>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    verify_document(&state, doc_id, workspace_id).await?;

    let now = Utc::now();

    let row: DocumentRow = sqlx::query_as(
        r#"
        UPDATE documents
        SET title = COALESCE($1, title),
            content = COALESCE($2, content),
            updated_at = $3
        WHERE id = $4
        RETURNING id, workspace_id, path::text, parent_id, title, slug, content,
                  created_by, created_at, updated_at
        "#,
    )
    .bind(&req.title)
    .bind(&req.content)
    .bind(now)
    .bind(doc_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(row_to_document(row)))
}

/// DELETE /api/v1/workspaces/:id/documents/:doc_id
pub async fn delete_document(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, doc_id)): Path<(Uuid, Uuid)>,
) -> Result<(), AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    // Delete document (children cascade automatically via FK)
    let result = sqlx::query("DELETE FROM documents WHERE id = $1 AND workspace_id = $2")
        .bind(doc_id)
        .bind(workspace_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(())
}

/// POST /api/v1/workspaces/:id/documents/:doc_id/move
pub async fn move_document(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
    Path((workspace_id, doc_id)): Path<(Uuid, Uuid)>,
    Json(req): Json<MoveDocumentRequest>,
) -> Result<Json<Document>, AppError> {
    let role = check_membership(&state, workspace_id, user.id).await?;

    if !role.can_edit() {
        return Err(AppError::Forbidden);
    }

    verify_document(&state, doc_id, workspace_id).await?;

    let mut tx = state.db.begin().await?;

    // Get current document info
    let (current_path, slug): (String, String) = sqlx::query_as(
        "SELECT path::text, slug FROM documents WHERE id = $1",
    )
    .bind(doc_id)
    .fetch_one(&mut *tx)
    .await?;

    // Prevent moving to self or descendant
    if let Some(new_parent_id) = req.parent_id {
        if new_parent_id == doc_id {
            return Err(AppError::Validation(
                "Cannot move document to itself".to_string(),
            ));
        }

        // Check if new parent is a descendant
        let is_descendant: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM documents WHERE id = $1 AND path <@ $2::ltree",
        )
        .bind(new_parent_id)
        .bind(&current_path)
        .fetch_optional(&mut *tx)
        .await?;

        if is_descendant.is_some() {
            return Err(AppError::Validation(
                "Cannot move document to its own descendant".to_string(),
            ));
        }
    }

    // Calculate new path
    let new_path = if let Some(new_parent_id) = req.parent_id {
        let parent: Option<(String,)> = sqlx::query_as(
            "SELECT path::text FROM documents WHERE id = $1 AND workspace_id = $2",
        )
        .bind(new_parent_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;

        match parent {
            Some((parent_path,)) => format!("{}.{}", parent_path, slug),
            None => {
                return Err(AppError::Validation(
                    "New parent document not found in this workspace".to_string(),
                ))
            }
        }
    } else {
        // Moving to root
        slug.clone()
    };

    // Check for path conflict
    let conflict: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM documents WHERE workspace_id = $1 AND path = $2::ltree AND id != $3",
    )
    .bind(workspace_id)
    .bind(&new_path)
    .bind(doc_id)
    .fetch_optional(&mut *tx)
    .await?;

    if conflict.is_some() {
        return Err(AppError::Conflict(
            "A document with this path already exists".to_string(),
        ));
    }

    let now = Utc::now();

    // Update all descendants' paths
    // Replace the old prefix with the new prefix
    sqlx::query(
        r#"
        UPDATE documents
        SET path = ($1::ltree || subpath(path, nlevel($2::ltree))),
            updated_at = $3
        WHERE workspace_id = $4 AND path <@ $2::ltree AND id != $5
        "#,
    )
    .bind(&new_path)
    .bind(&current_path)
    .bind(now)
    .bind(workspace_id)
    .bind(doc_id)
    .execute(&mut *tx)
    .await?;

    // Update the document itself
    let row: DocumentRow = sqlx::query_as(
        r#"
        UPDATE documents
        SET parent_id = $1,
            path = $2::ltree,
            updated_at = $3
        WHERE id = $4
        RETURNING id, workspace_id, path::text, parent_id, title, slug, content,
                  created_by, created_at, updated_at
        "#,
    )
    .bind(req.parent_id)
    .bind(&new_path)
    .bind(now)
    .bind(doc_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(row_to_document(row)))
}
