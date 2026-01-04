use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,        // User ID
    pub email: String,
    pub exp: i64,         // Expiration timestamp
    pub iat: i64,         // Issued at timestamp
}

pub fn create_access_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
    expires_in_secs: i64,
) -> Result<String, AppError> {
    let now = Utc::now();
    let exp = now + Duration::seconds(expires_in_secs);

    let claims = Claims {
        sub: user_id,
        email: email.to_string(),
        exp: exp.timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to create token: {}", e)))
}

pub fn create_refresh_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
    expires_in_secs: i64,
) -> Result<String, AppError> {
    // Refresh tokens have longer expiry
    create_access_token(user_id, email, secret, expires_in_secs)
}

pub fn verify_access_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| {
        tracing::debug!("Token verification failed: {}", e);
        AppError::Unauthorized
    })?;

    Ok(token_data.claims)
}
