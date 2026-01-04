use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub user_id: Uuid,
}

impl AuthTokens {
    /// Get the path to the auth token file
    fn token_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("todo-tui");

        fs::create_dir_all(&config_dir)
            .context("Could not create config directory")?;

        Ok(config_dir.join("auth.json"))
    }

    /// Load tokens from disk
    pub fn load() -> Result<Option<Self>> {
        let path = Self::token_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .context("Could not read auth file")?;

        let tokens: Self = serde_json::from_str(&contents)
            .context("Could not parse auth file")?;

        Ok(Some(tokens))
    }

    /// Save tokens to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::token_path()?;
        let contents = serde_json::to_string_pretty(self)
            .context("Could not serialize tokens")?;

        fs::write(&path, contents)
            .context("Could not write auth file")?;

        Ok(())
    }

    /// Delete stored tokens
    pub fn delete() -> Result<()> {
        let path = Self::token_path()?;

        if path.exists() {
            fs::remove_file(&path)
                .context("Could not delete auth file")?;
        }

        Ok(())
    }
}
