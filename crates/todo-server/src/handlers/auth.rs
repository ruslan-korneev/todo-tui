use axum::{extract::State, Extension, Json};
use chrono::Utc;
use todo_shared::api::{AuthResponse, LoginRequest, RefreshRequest, RegisterRequest};
use todo_shared::User;
use uuid::Uuid;

use crate::auth::{create_access_token, create_refresh_token, hash_password, verify_password, AuthUser};
use crate::error::AppError;
use crate::routes::AppState;

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Validate input
    if req.email.is_empty() || req.password.is_empty() || req.display_name.is_empty() {
        return Err(AppError::Validation("All fields are required".to_string()));
    }

    if req.password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check if email already exists
    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    // Hash password and create user
    let password_hash = hash_password(&req.password)?;
    let user_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, display_name)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.display_name)
    .execute(&state.db)
    .await?;

    // Generate tokens
    let access_token = create_access_token(
        user_id,
        &req.email,
        &state.config.jwt_secret,
        state.config.jwt_expires_in,
    )?;

    let refresh_token = create_refresh_token(
        user_id,
        &req.email,
        &state.config.jwt_secret,
        state.config.refresh_token_expires_in,
    )?;

    // Store refresh token hash
    let token_hash = hash_password(&refresh_token)?;
    let expires_at = Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expires_in);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user_id,
    }))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Find user by email
    let row: Option<(Uuid, String, String)> = sqlx::query_as(
        "SELECT id, email, password_hash FROM users WHERE email = $1"
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, email, password_hash) = row.ok_or(AppError::Unauthorized)?;

    // Verify password
    if !verify_password(&req.password, &password_hash)? {
        return Err(AppError::Unauthorized);
    }

    // Update last login
    sqlx::query("UPDATE users SET last_login_at = NOW() WHERE id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;

    // Generate tokens
    let access_token = create_access_token(
        user_id,
        &email,
        &state.config.jwt_secret,
        state.config.jwt_expires_in,
    )?;

    let refresh_token = create_refresh_token(
        user_id,
        &email,
        &state.config.jwt_secret,
        state.config.refresh_token_expires_in,
    )?;

    // Store refresh token
    let token_hash = hash_password(&refresh_token)?;
    let expires_at = Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expires_in);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user_id,
    }))
}

pub async fn refresh(
    State(state): State<AppState>,
    Json(req): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Verify the refresh token JWT
    let claims = crate::auth::verify_access_token(&req.refresh_token, &state.config.jwt_secret)?;

    // Check if token exists and not revoked
    let row: Option<(Uuid, String)> = sqlx::query_as(
        r#"
        SELECT rt.id, u.email
        FROM refresh_tokens rt
        JOIN users u ON u.id = rt.user_id
        WHERE rt.user_id = $1
          AND rt.revoked_at IS NULL
          AND rt.expires_at > NOW()
        ORDER BY rt.created_at DESC
        LIMIT 1
        "#,
    )
    .bind(claims.sub)
    .fetch_optional(&state.db)
    .await?;

    let (token_id, email) = row.ok_or(AppError::Unauthorized)?;

    // Revoke old refresh token
    sqlx::query("UPDATE refresh_tokens SET revoked_at = NOW() WHERE id = $1")
        .bind(token_id)
        .execute(&state.db)
        .await?;

    // Generate new tokens
    let access_token = create_access_token(
        claims.sub,
        &email,
        &state.config.jwt_secret,
        state.config.jwt_expires_in,
    )?;

    let refresh_token = create_refresh_token(
        claims.sub,
        &email,
        &state.config.jwt_secret,
        state.config.refresh_token_expires_in,
    )?;

    // Store new refresh token
    let token_hash = hash_password(&refresh_token)?;
    let expires_at = Utc::now() + chrono::Duration::seconds(state.config.refresh_token_expires_in);

    sqlx::query(
        r#"
        INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(claims.sub)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    Ok(Json(AuthResponse {
        access_token,
        refresh_token,
        user_id: claims.sub,
    }))
}

pub async fn logout(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<(), AppError> {
    // Revoke all refresh tokens for user
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
    )
    .bind(user.id)
    .execute(&state.db)
    .await?;

    Ok(())
}

pub async fn me(
    State(state): State<AppState>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<User>, AppError> {
    let row: Option<(Uuid, String, String, Option<String>, chrono::DateTime<Utc>, chrono::DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT id, email, display_name, avatar_url, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(user.id)
        .fetch_optional(&state.db)
        .await?;

    let (id, email, display_name, avatar_url, created_at, updated_at) =
        row.ok_or(AppError::NotFound)?;

    Ok(Json(User {
        id,
        email,
        display_name,
        avatar_url,
        created_at,
        updated_at,
    }))
}
