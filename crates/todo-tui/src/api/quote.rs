use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Cached quote with timestamp
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedQuote {
    pub quote: String,
    pub author: String,
    pub date: String, // YYYY-MM-DD format
}

impl CachedQuote {
    fn cache_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
            .join("todo");

        fs::create_dir_all(&config_dir)?;
        Ok(config_dir.join("quote.json"))
    }

    /// Load cached quote from disk
    pub fn load() -> Result<Option<Self>> {
        let path = Self::cache_path()?;

        if !path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&path)?;
        let cached: Self = serde_json::from_str(&contents)?;

        Ok(Some(cached))
    }

    /// Save quote to disk
    pub fn save(&self) -> Result<()> {
        let path = Self::cache_path()?;
        let contents = serde_json::to_string_pretty(self)?;
        fs::write(&path, contents)?;
        Ok(())
    }
}

/// Fetch quote from ZenQuotes API
pub async fn fetch_quote_of_day() -> Result<(String, String)> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    // Check cache first
    if let Ok(Some(cached)) = CachedQuote::load() {
        if cached.date == today {
            return Ok((cached.quote, cached.author));
        }
    }

    // Fetch from API
    let client = reqwest::Client::new();
    let response = client
        .get("https://zenquotes.io/api/today")
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    #[derive(Deserialize)]
    struct ZenQuote {
        q: String,
        a: String,
    }

    let quotes: Vec<ZenQuote> = response.json().await?;

    if let Some(quote) = quotes.first() {
        // Cache the quote
        let cached = CachedQuote {
            quote: quote.q.clone(),
            author: quote.a.clone(),
            date: today,
        };
        let _ = cached.save(); // Ignore cache save errors

        Ok((quote.q.clone(), quote.a.clone()))
    } else {
        anyhow::bail!("No quote returned from API")
    }
}

/// Get quote (from cache or API), with fallback
pub async fn get_quote() -> (String, String) {
    match fetch_quote_of_day().await {
        Ok((quote, author)) => (quote, author),
        Err(_) => {
            // Try to use stale cache as fallback
            if let Ok(Some(cached)) = CachedQuote::load() {
                (cached.quote, cached.author)
            } else {
                // Hardcoded fallback quote
                (
                    "The only way to do great work is to love what you do.".to_string(),
                    "Steve Jobs".to_string(),
                )
            }
        }
    }
}
