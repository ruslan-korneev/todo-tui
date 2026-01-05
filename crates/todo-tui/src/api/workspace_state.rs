use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub workspace_id: Uuid,
}

impl WorkspaceState {
    /// Get the path to the workspace state file
    fn state_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("todo");

        fs::create_dir_all(&config_dir)
            .context("Could not create config directory")?;

        Ok(config_dir.join("workspace.json"))
    }

    /// Load workspace state from disk
    pub fn load() -> Result<Option<Self>> {
        let path = Self::state_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)
            .context("Could not read workspace state file")?;

        let state: Self = serde_json::from_str(&contents)
            .context("Could not parse workspace state file")?;

        Ok(Some(state))
    }

    /// Save workspace state to disk
    pub fn save(workspace_id: Uuid) -> Result<()> {
        let state = Self { workspace_id };
        let path = Self::state_path()?;
        let contents = serde_json::to_string_pretty(&state)
            .context("Could not serialize workspace state")?;

        fs::write(&path, contents)
            .context("Could not write workspace state file")?;

        Ok(())
    }
}
