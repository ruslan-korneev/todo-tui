use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::app::FilterPreset;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserPreferences {
    pub filter_presets: Vec<FilterPreset>,
}

impl UserPreferences {
    /// Get the path to the preferences file
    fn prefs_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not find config directory")?
            .join("todo");

        fs::create_dir_all(&config_dir)
            .context("Could not create config directory")?;

        Ok(config_dir.join("preferences.json"))
    }

    /// Load preferences from disk
    pub fn load() -> Result<Self> {
        let path = Self::prefs_path()?;

        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .context("Could not read preferences file")?;

        let prefs: Self = serde_json::from_str(&contents)
            .context("Could not parse preferences file")?;

        Ok(prefs)
    }

    /// Save preferences to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::prefs_path()?;
        let contents = serde_json::to_string_pretty(self)
            .context("Could not serialize preferences")?;

        fs::write(&path, contents)
            .context("Could not write preferences file")?;

        Ok(())
    }
}
