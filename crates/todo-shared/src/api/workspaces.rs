use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::WorkspaceRole;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkspaceRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateWorkspaceRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteMemberRequest {
    pub email: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateMemberRoleRequest {
    pub role: WorkspaceRole,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateStatusRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub is_done: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateStatusRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_done: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReorderStatusesRequest {
    pub status_ids: Vec<Uuid>,
}
