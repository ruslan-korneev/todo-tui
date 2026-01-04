use serde::{Deserialize, Serialize};

use crate::models::Task;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SearchParams {
    pub q: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuzzy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchResultItem {
    Task(SearchTaskResult),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchTaskResult {
    pub task: Task,
    pub rank: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_highlights: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_highlights: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
    pub query: String,
}
