use serde::{Deserialize, Serialize};

use crate::models::{Document, Task};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SearchType {
    #[default]
    All,
    Tasks,
    Documents,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct SearchParams {
    pub q: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fuzzy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_type: Option<SearchType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchResultItem {
    Task(SearchTaskResult),
    Document(SearchDocumentResult),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchDocumentResult {
    pub document: Document,
    pub rank: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_highlights: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_highlights: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultItem>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
    pub query: String,
}
