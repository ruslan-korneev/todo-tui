use axum::{extract::State, Extension, Json};
use chrono::Utc;
use rand::Rng;
use regex::Regex;
use todo_shared::api::{
    AuthResponse, LoginRequest, RefreshRequest, RegisterRequest, RegisterResponse,
    ResendVerificationRequest, VerifyEmailRequest,
};
use todo_shared::User;
use uuid::Uuid;

use crate::auth::{
    create_access_token, create_refresh_token, hash_password, verify_password, AuthUser,
};
use crate::error::AppError;
use crate::routes::AppState;

/// Generate a random 6-digit verification code
fn generate_verification_code() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..1000000))
}

/// Validate username format: 3-30 chars, alphanumeric + underscore, starts with letter
fn validate_username(username: &str) -> Result<(), AppError> {
    if username.len() < 3 || username.len() > 30 {
        return Err(AppError::Validation(
            "Username must be between 3 and 30 characters".to_string(),
        ));
    }

    let re = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_]*$").unwrap();
    if !re.is_match(username) {
        return Err(AppError::Validation(
            "Username must start with a letter and contain only letters, numbers, and underscores"
                .to_string(),
        ));
    }

    Ok(())
}

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<RegisterResponse>, AppError> {
    // Validate input
    if req.email.is_empty() || req.password.is_empty() || req.display_name.is_empty() {
        return Err(AppError::Validation("All fields are required".to_string()));
    }

    if req.username.is_empty() {
        return Err(AppError::Validation("Username is required".to_string()));
    }

    validate_username(&req.username)?;

    if req.password.len() < 8 {
        return Err(AppError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }

    // Check if email already exists
    let existing_email: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(&req.email)
            .fetch_optional(&state.db)
            .await?;

    if existing_email.is_some() {
        return Err(AppError::Conflict("Email already registered".to_string()));
    }

    // Check if username already exists (case-insensitive)
    let existing_username: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE LOWER(username) = LOWER($1)")
            .bind(&req.username)
            .fetch_optional(&state.db)
            .await?;

    if existing_username.is_some() {
        return Err(AppError::Conflict("Username already taken".to_string()));
    }

    // Hash password and create user
    let password_hash = hash_password(&req.password)?;
    let user_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO users (id, username, email, password_hash, display_name, email_verified)
        VALUES ($1, $2, $3, $4, $5, FALSE)
        "#,
    )
    .bind(user_id)
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.display_name)
    .execute(&state.db)
    .await?;

    // Generate verification code
    let code = generate_verification_code();
    let expires_at = Utc::now() + chrono::Duration::minutes(15);

    sqlx::query(
        r#"
        INSERT INTO email_verification_codes (user_id, code, expires_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(&code)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    // Create default workspace for the user
    let workspace_id = Uuid::new_v4();
    let workspace_slug = format!("personal-{}", &user_id.to_string()[..8]);

    sqlx::query(
        r#"
        INSERT INTO workspaces (id, name, slug, owner_id, is_default)
        VALUES ($1, 'Personal', $2, $3, TRUE)
        "#,
    )
    .bind(workspace_id)
    .bind(&workspace_slug)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    // Add user as owner of the workspace
    sqlx::query(
        r#"
        INSERT INTO workspace_members (workspace_id, user_id, role)
        VALUES ($1, $2, 'owner')
        "#,
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&state.db)
    .await?;

    // Create default statuses for the workspace
    let status_ids: Vec<Uuid> = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    sqlx::query(
        r#"
        INSERT INTO task_statuses (id, workspace_id, name, slug, color, position, is_done)
        VALUES
            ($1, $4, 'To Do', 'todo', '#6B7280', 0, FALSE),
            ($2, $4, 'In Progress', 'in-progress', '#3B82F6', 1, FALSE),
            ($3, $4, 'Done', 'done', '#10B981', 2, TRUE)
        "#,
    )
    .bind(status_ids[0])
    .bind(status_ids[1])
    .bind(status_ids[2])
    .bind(workspace_id)
    .execute(&state.db)
    .await?;

    // Log verification code to console (development mode)
    tracing::info!(
        "VERIFICATION CODE for {} ({}): {}",
        req.email,
        req.username,
        code
    );

    Ok(Json(RegisterResponse {
        user_id,
        email: req.email,
        message: "Registration successful. Please check your email for verification code."
            .to_string(),
    }))
}

pub async fn verify_email(
    State(state): State<AppState>,
    Json(req): Json<VerifyEmailRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Find user by email
    let user_row: Option<(Uuid, String, bool)> =
        sqlx::query_as("SELECT id, email, email_verified FROM users WHERE email = $1")
            .bind(&req.email)
            .fetch_optional(&state.db)
            .await?;

    let (user_id, email, email_verified) = user_row.ok_or(AppError::NotFound)?;

    if email_verified {
        return Err(AppError::Validation("Email already verified".to_string()));
    }

    // Find valid verification code
    let code_row: Option<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT id FROM email_verification_codes
        WHERE user_id = $1 AND code = $2 AND expires_at > NOW() AND used_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(&req.code)
    .fetch_optional(&state.db)
    .await?;

    let (code_id,) = code_row.ok_or(AppError::Validation(
        "Invalid or expired verification code".to_string(),
    ))?;

    // Mark code as used
    sqlx::query("UPDATE email_verification_codes SET used_at = NOW() WHERE id = $1")
        .bind(code_id)
        .execute(&state.db)
        .await?;

    // Set email as verified
    sqlx::query("UPDATE users SET email_verified = TRUE, email_verified_at = NOW() WHERE id = $1")
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

pub async fn resend_verification(
    State(state): State<AppState>,
    Json(req): Json<ResendVerificationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Find user by email
    let user_row: Option<(Uuid, String, bool)> =
        sqlx::query_as("SELECT id, username, email_verified FROM users WHERE email = $1")
            .bind(&req.email)
            .fetch_optional(&state.db)
            .await?;

    let (user_id, username, email_verified) = user_row.ok_or(AppError::NotFound)?;

    if email_verified {
        return Err(AppError::Validation("Email already verified".to_string()));
    }

    // Invalidate old codes
    sqlx::query("UPDATE email_verification_codes SET used_at = NOW() WHERE user_id = $1 AND used_at IS NULL")
        .bind(user_id)
        .execute(&state.db)
        .await?;

    // Generate new code
    let code = generate_verification_code();
    let expires_at = Utc::now() + chrono::Duration::minutes(15);

    sqlx::query(
        r#"
        INSERT INTO email_verification_codes (user_id, code, expires_at)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(user_id)
    .bind(&code)
    .bind(expires_at)
    .execute(&state.db)
    .await?;

    // Log verification code to console (development mode)
    tracing::info!(
        "VERIFICATION CODE for {} ({}): {}",
        req.email,
        username,
        code
    );

    Ok(Json(serde_json::json!({
        "message": "Verification code sent"
    })))
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Find user by email
    let row: Option<(Uuid, String, String, bool)> = sqlx::query_as(
        "SELECT id, email, password_hash, email_verified FROM users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, email, password_hash, email_verified) = row.ok_or(AppError::Unauthorized)?;

    // Verify password
    if !verify_password(&req.password, &password_hash)? {
        return Err(AppError::Unauthorized);
    }

    // Check if email is verified
    if !email_verified {
        return Err(AppError::EmailNotVerified);
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
    let row: Option<(
        Uuid,
        String,
        String,
        String,
        Option<String>,
        bool,
        chrono::DateTime<Utc>,
        chrono::DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT id, username, email, display_name, avatar_url, email_verified, created_at, updated_at FROM users WHERE id = $1",
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    let (id, username, email, display_name, avatar_url, email_verified, created_at, updated_at) =
        row.ok_or(AppError::NotFound)?;

    Ok(Json(User {
        id,
        username,
        email,
        display_name,
        avatar_url,
        email_verified,
        created_at,
        updated_at,
    }))
}
