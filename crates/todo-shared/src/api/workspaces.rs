use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::{WorkspaceRole, WorkspaceSettings};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<WorkspaceSettings>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceMemberWithUser {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub role: WorkspaceRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInvite {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub email: String,
    pub role: WorkspaceRole,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteDetails {
    pub workspace_name: String,
    pub inviter_name: String,
    pub role: WorkspaceRole,
    pub expires_at: DateTime<Utc>,
}
