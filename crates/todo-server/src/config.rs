use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expires_in: i64,
    pub refresh_token_expires_in: i64,
    pub port: u16,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set"),
            jwt_secret: env::var("JWT_SECRET")
                .expect("JWT_SECRET must be set"),
            jwt_expires_in: env::var("JWT_EXPIRES_IN")
                .unwrap_or_else(|_| "900".to_string()) // 15 minutes
                .parse()?,
            refresh_token_expires_in: env::var("REFRESH_TOKEN_EXPIRES_IN")
                .unwrap_or_else(|_| "604800".to_string()) // 7 days
                .parse()?,
            port: env::var("PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()?,
        })
    }
}
