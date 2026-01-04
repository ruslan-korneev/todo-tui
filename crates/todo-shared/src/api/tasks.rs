use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::Priority;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub status_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_estimate_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_estimate_minutes: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MoveTaskRequest {
    pub status_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned_to: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_before: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub due_after: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCommentRequest {
    pub content: String,
}
