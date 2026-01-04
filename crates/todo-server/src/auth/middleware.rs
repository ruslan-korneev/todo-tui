use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{error::AppError, routes::AppState};

use super::jwt::verify_access_token;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;

    let claims = verify_access_token(token, &state.config.jwt_secret)?;

    let auth_user = AuthUser {
        id: claims.sub,
        email: claims.email,
    };

    request.extensions_mut().insert(auth_user);

    Ok(next.run(request).await)
}
