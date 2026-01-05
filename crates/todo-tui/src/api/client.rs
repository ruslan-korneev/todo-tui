use anyhow::Result;
use reqwest::{Client, StatusCode};
use todo_shared::{
    api::{
        AuthResponse, CreateCommentRequest, CreateDocumentRequest, CreateStatusRequest,
        CreateTagRequest, CreateTaskRequest, CreateWorkspaceRequest, InviteDetails, LoginRequest,
        MoveTaskRequest, RefreshRequest, RegisterRequest, RegisterResponse,
        ResendVerificationRequest, SearchResponse, SetTaskTagsRequest, TaskListParams,
        UpdateCommentRequest, UpdateDocumentRequest, UpdateStatusRequest, UpdateTagRequest,
        UpdateTaskRequest, UpdateWorkspaceRequest, VerifyEmailRequest, WorkspaceInvite,
        WorkspaceMemberWithUser,
    },
    CommentWithAuthor, Document, Tag, Task, TaskStatus, User, Workspace, WorkspaceRole,
    WorkspaceSettings, WorkspaceWithRole,
};
use uuid::Uuid;

use super::auth::AuthTokens;

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)] // Pagination fields for future use
pub struct TaskListResponse {
    pub tasks: Vec<Task>,
    pub total: i64,
    pub page: u32,
    pub limit: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Not authenticated")]
    Unauthorized,
    #[error("Access forbidden")]
    Forbidden,
    #[error("Email not verified")]
    EmailNotVerified,
    #[error("Resource not found")]
    NotFound,
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Server error: {0}")]
    Server(String),
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub struct ApiClient {
    client: Client,
    base_url: String,
    tokens: Option<AuthTokens>,
}

#[allow(dead_code)] // API methods scaffolded for future TUI features
impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            tokens: None,
        }
    }

    /// Load tokens from disk
    pub fn load_tokens(&mut self) -> Result<bool> {
        self.tokens = AuthTokens::load()?;
        Ok(self.tokens.is_some())
    }

    /// Check if authenticated
    pub fn is_authenticated(&self) -> bool {
        self.tokens.is_some()
    }

    /// Get current user ID
    pub fn user_id(&self) -> Option<Uuid> {
        self.tokens.as_ref().map(|t| t.user_id)
    }

    /// Build URL for endpoint
    fn url(&self, path: &str) -> String {
        format!("{}/api/v1{}", self.base_url, path)
    }

    /// Add auth header if authenticated
    fn auth_header(&self) -> Option<String> {
        self.tokens
            .as_ref()
            .map(|t| format!("Bearer {}", t.access_token))
    }

    /// Handle API response
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, ApiError> {
        let status = response.status();

        match status {
            StatusCode::OK | StatusCode::CREATED => {
                response.json().await.map_err(ApiError::Network)
            }
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized),
            StatusCode::FORBIDDEN => {
                let text = response.text().await.unwrap_or_default();
                if text.contains("Email not verified") {
                    Err(ApiError::EmailNotVerified)
                } else {
                    Err(ApiError::Forbidden)
                }
            }
            StatusCode::NOT_FOUND => Err(ApiError::NotFound),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                let text = response.text().await.unwrap_or_default();
                Err(ApiError::Validation(text))
            }
            _ => {
                let text = response.text().await.unwrap_or_default();
                Err(ApiError::Server(format!("{}: {}", status, text)))
            }
        }
    }

    /// Handle empty response
    async fn handle_empty_response(&self, response: reqwest::Response) -> Result<(), ApiError> {
        let status = response.status();

        match status {
            StatusCode::OK | StatusCode::NO_CONTENT => Ok(()),
            StatusCode::UNAUTHORIZED => Err(ApiError::Unauthorized),
            StatusCode::FORBIDDEN => {
                let text = response.text().await.unwrap_or_default();
                if text.contains("Email not verified") {
                    Err(ApiError::EmailNotVerified)
                } else {
                    Err(ApiError::Forbidden)
                }
            }
            StatusCode::NOT_FOUND => Err(ApiError::NotFound),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                let text = response.text().await.unwrap_or_default();
                Err(ApiError::Validation(text))
            }
            _ => {
                let text = response.text().await.unwrap_or_default();
                Err(ApiError::Server(format!("{}: {}", status, text)))
            }
        }
    }

    // ============ Auth ============

    pub async fn register(
        &mut self,
        username: &str,
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<RegisterResponse, ApiError> {
        let req = RegisterRequest {
            username: username.to_string(),
            email: email.to_string(),
            password: password.to_string(),
            display_name: display_name.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/api/v1/auth/register", self.base_url))
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn verify_email(&mut self, email: &str, code: &str) -> Result<User, ApiError> {
        let req = VerifyEmailRequest {
            email: email.to_string(),
            code: code.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/api/v1/auth/verify-email", self.base_url))
            .json(&req)
            .send()
            .await?;

        let auth: AuthResponse = self.handle_response(response).await?;

        // Store tokens
        self.tokens = Some(AuthTokens {
            access_token: auth.access_token,
            refresh_token: auth.refresh_token,
            user_id: auth.user_id,
        });

        if let Some(ref tokens) = self.tokens {
            tokens.save().map_err(ApiError::Other)?;
        }

        // Fetch user details
        self.me().await
    }

    pub async fn resend_verification(&self, email: &str) -> Result<(), ApiError> {
        let req = ResendVerificationRequest {
            email: email.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/api/v1/auth/resend-verification", self.base_url))
            .json(&req)
            .send()
            .await?;

        // Just check for success, ignore the response body
        let status = response.status();
        match status {
            StatusCode::OK => Ok(()),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => {
                let text = response.text().await.unwrap_or_default();
                Err(ApiError::Validation(text))
            }
            StatusCode::NOT_FOUND => Err(ApiError::NotFound),
            _ => {
                let text = response.text().await.unwrap_or_default();
                Err(ApiError::Server(format!("{}: {}", status, text)))
            }
        }
    }

    pub async fn login(&mut self, email: &str, password: &str) -> Result<User, ApiError> {
        let req = LoginRequest {
            email: email.to_string(),
            password: password.to_string(),
        };

        let response = self
            .client
            .post(&format!("{}/api/v1/auth/login", self.base_url))
            .json(&req)
            .send()
            .await?;

        let auth: AuthResponse = self.handle_response(response).await?;

        // Store tokens
        self.tokens = Some(AuthTokens {
            access_token: auth.access_token,
            refresh_token: auth.refresh_token,
            user_id: auth.user_id,
        });

        // Save to disk
        if let Some(ref tokens) = self.tokens {
            tokens.save().map_err(ApiError::Other)?;
        }

        // Fetch user details
        self.me().await
    }

    pub async fn logout(&mut self) -> Result<(), ApiError> {
        if let Some(ref auth) = self.auth_header() {
            let _ = self
                .client
                .post(&self.url("/auth/logout"))
                .header("Authorization", auth)
                .send()
                .await;
        }

        self.tokens = None;
        AuthTokens::delete().map_err(ApiError::Other)?;
        Ok(())
    }

    pub async fn refresh(&mut self) -> Result<(), ApiError> {
        let refresh_token = self
            .tokens
            .as_ref()
            .map(|t| t.refresh_token.clone())
            .ok_or(ApiError::Unauthorized)?;

        let req = RefreshRequest { refresh_token };

        let response = self
            .client
            .post(&format!("{}/api/v1/auth/refresh", self.base_url))
            .json(&req)
            .send()
            .await?;

        let auth: AuthResponse = self.handle_response(response).await?;

        self.tokens = Some(AuthTokens {
            access_token: auth.access_token,
            refresh_token: auth.refresh_token,
            user_id: auth.user_id,
        });

        if let Some(ref tokens) = self.tokens {
            tokens.save().map_err(ApiError::Other)?;
        }

        Ok(())
    }

    pub async fn me(&self) -> Result<User, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url("/auth/me"))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    // ============ Workspaces ============

    pub async fn list_workspaces(&self) -> Result<Vec<WorkspaceWithRole>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url("/workspaces"))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn create_workspace(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<Workspace, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = CreateWorkspaceRequest {
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
        };

        let response = self
            .client
            .post(&self.url("/workspaces"))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn get_workspace(&self, id: Uuid) -> Result<WorkspaceWithRole, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!("/workspaces/{}", id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_workspace(
        &self,
        id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        settings: Option<WorkspaceSettings>,
    ) -> Result<Workspace, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = UpdateWorkspaceRequest {
            name: name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            settings,
        };

        let response = self
            .client
            .patch(&self.url(&format!("/workspaces/{}", id)))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn list_members(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceMemberWithUser>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!("/workspaces/{}/members", workspace_id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn delete_workspace(&self, id: Uuid) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(&self.url(&format!("/workspaces/{}", id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    // ============ Member Management ============

    pub async fn create_invite(
        &self,
        workspace_id: Uuid,
        email: &str,
        role: WorkspaceRole,
    ) -> Result<WorkspaceInvite, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .post(self.url(&format!("/workspaces/{}/invites", workspace_id)))
            .header("Authorization", &auth)
            .json(&serde_json::json!({
                "email": email,
                "role": role
            }))
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn get_invite(&self, token: &str) -> Result<InviteDetails, ApiError> {
        let response = self
            .client
            .get(self.url(&format!("/invites/{}", token)))
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn accept_invite(&self, token: &str) -> Result<WorkspaceWithRole, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .post(self.url(&format!("/invites/{}/accept", token)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_member_role(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: WorkspaceRole,
    ) -> Result<WorkspaceMemberWithUser, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .put(self.url(&format!(
                "/workspaces/{}/members/{}",
                workspace_id, user_id
            )))
            .header("Authorization", &auth)
            .json(&serde_json::json!({ "role": role }))
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn remove_member(&self, workspace_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(self.url(&format!(
                "/workspaces/{}/members/{}",
                workspace_id, user_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    // ============ Statuses ============

    pub async fn list_statuses(&self, workspace_id: Uuid) -> Result<Vec<TaskStatus>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!("/workspaces/{}/statuses", workspace_id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn create_status(
        &self,
        workspace_id: Uuid,
        name: &str,
        color: Option<&str>,
        is_done: bool,
    ) -> Result<TaskStatus, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = CreateStatusRequest {
            name: name.to_string(),
            color: color.map(|s| s.to_string()),
            is_done,
        };

        let response = self
            .client
            .post(&self.url(&format!("/workspaces/{}/statuses", workspace_id)))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_status(
        &self,
        workspace_id: Uuid,
        status_id: Uuid,
        name: Option<&str>,
        color: Option<&str>,
        is_done: Option<bool>,
    ) -> Result<TaskStatus, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = UpdateStatusRequest {
            name: name.map(|s| s.to_string()),
            color: color.map(|s| s.to_string()),
            is_done,
        };

        let response = self
            .client
            .patch(&self.url(&format!(
                "/workspaces/{}/statuses/{}",
                workspace_id, status_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn delete_status(
        &self,
        workspace_id: Uuid,
        status_id: Uuid,
    ) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(&self.url(&format!(
                "/workspaces/{}/statuses/{}",
                workspace_id, status_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    pub async fn reorder_statuses(
        &self,
        workspace_id: Uuid,
        status_ids: Vec<Uuid>,
    ) -> Result<Vec<TaskStatus>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        #[derive(serde::Serialize)]
        struct ReorderRequest {
            status_ids: Vec<Uuid>,
        }

        let response = self
            .client
            .post(&self.url(&format!("/workspaces/{}/statuses/reorder", workspace_id)))
            .header("Authorization", &auth)
            .json(&ReorderRequest { status_ids })
            .send()
            .await?;

        self.handle_response(response).await
    }

    // ============ Tasks ============

    pub async fn list_tasks(
        &self,
        workspace_id: Uuid,
        params: Option<&TaskListParams>,
    ) -> Result<TaskListResponse, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let mut url = self.url(&format!("/workspaces/{}/tasks", workspace_id));

        // Build query string from TaskListParams
        if let Some(params) = params {
            let mut query_parts = Vec::new();

            if let Some(status_id) = &params.status_id {
                query_parts.push(format!("status_id={}", status_id));
            }
            if let Some(priority) = &params.priority {
                query_parts.push(format!("priority={}", serde_json::to_string(priority).unwrap_or_default().trim_matches('"')));
            }
            if let Some(assigned_to) = &params.assigned_to {
                query_parts.push(format!("assigned_to={}", assigned_to));
            }
            if let Some(due_before) = &params.due_before {
                query_parts.push(format!("due_before={}", due_before));
            }
            if let Some(due_after) = &params.due_after {
                query_parts.push(format!("due_after={}", due_after));
            }
            if let Some(q) = &params.q {
                query_parts.push(format!("q={}", urlencoding::encode(q)));
            }
            if let Some(tag_ids) = &params.tag_ids {
                let ids: Vec<String> = tag_ids.iter().map(|id| id.to_string()).collect();
                query_parts.push(format!("tag_ids={}", ids.join(",")));
            }
            if let Some(order_by) = &params.order_by {
                query_parts.push(format!("order_by={}", order_by));
            }
            if let Some(order) = &params.order {
                query_parts.push(format!("order={}", order));
            }
            if let Some(page) = &params.page {
                query_parts.push(format!("page={}", page));
            }
            if let Some(limit) = &params.limit {
                query_parts.push(format!("limit={}", limit));
            }

            if !query_parts.is_empty() {
                url.push_str("?");
                url.push_str(&query_parts.join("&"));
            }
        }

        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn create_task(
        &self,
        workspace_id: Uuid,
        req: CreateTaskRequest,
    ) -> Result<Task, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .post(&self.url(&format!("/workspaces/{}/tasks", workspace_id)))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn get_task(&self, workspace_id: Uuid, task_id: Uuid) -> Result<Task, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!(
                "/workspaces/{}/tasks/{}",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_task(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
        req: UpdateTaskRequest,
    ) -> Result<Task, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .patch(&self.url(&format!(
                "/workspaces/{}/tasks/{}",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn delete_task(&self, workspace_id: Uuid, task_id: Uuid) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(&self.url(&format!(
                "/workspaces/{}/tasks/{}",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    pub async fn move_task(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
        status_id: Uuid,
        position: Option<i32>,
    ) -> Result<Task, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = MoveTaskRequest {
            status_id,
            position,
        };

        let response = self
            .client
            .post(&self.url(&format!(
                "/workspaces/{}/tasks/{}/move",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    // ============ Search ============

    pub async fn search(
        &self,
        workspace_id: Uuid,
        query: &str,
        fuzzy: bool,
        page: Option<u32>,
        limit: Option<u32>,
    ) -> Result<SearchResponse, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let mut url = self.url(&format!("/workspaces/{}/search", workspace_id));
        url.push_str(&format!("?q={}", urlencoding::encode(query)));

        if fuzzy {
            url.push_str("&fuzzy=true");
        }
        if let Some(p) = page {
            url.push_str(&format!("&page={}", p));
        }
        if let Some(l) = limit {
            url.push_str(&format!("&limit={}", l));
        }

        let response = self
            .client
            .get(&url)
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    // ============ Comments ============

    pub async fn list_comments(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<CommentWithAuthor>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!(
                "/workspaces/{}/tasks/{}/comments",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn create_comment(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
        content: &str,
    ) -> Result<CommentWithAuthor, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = CreateCommentRequest {
            content: content.to_string(),
        };

        let response = self
            .client
            .post(&self.url(&format!(
                "/workspaces/{}/tasks/{}/comments",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_comment(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
        comment_id: Uuid,
        content: &str,
    ) -> Result<CommentWithAuthor, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = UpdateCommentRequest {
            content: content.to_string(),
        };

        let response = self
            .client
            .patch(&self.url(&format!(
                "/workspaces/{}/tasks/{}/comments/{}",
                workspace_id, task_id, comment_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn delete_comment(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
        comment_id: Uuid,
    ) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(&self.url(&format!(
                "/workspaces/{}/tasks/{}/comments/{}",
                workspace_id, task_id, comment_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    // ============ Tags ============

    pub async fn list_tags(&self, workspace_id: Uuid) -> Result<Vec<Tag>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!("/workspaces/{}/tags", workspace_id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn create_tag(
        &self,
        workspace_id: Uuid,
        name: &str,
        color: Option<&str>,
    ) -> Result<Tag, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = CreateTagRequest {
            name: name.to_string(),
            color: color.map(|c| c.to_string()),
        };

        let response = self
            .client
            .post(&self.url(&format!("/workspaces/{}/tags", workspace_id)))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_tag(
        &self,
        workspace_id: Uuid,
        tag_id: Uuid,
        name: Option<&str>,
        color: Option<&str>,
    ) -> Result<Tag, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = UpdateTagRequest {
            name: name.map(|n| n.to_string()),
            color: color.map(|c| c.to_string()),
        };

        let response = self
            .client
            .patch(&self.url(&format!("/workspaces/{}/tags/{}", workspace_id, tag_id)))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn delete_tag(&self, workspace_id: Uuid, tag_id: Uuid) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(&self.url(&format!("/workspaces/{}/tags/{}", workspace_id, tag_id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }

    pub async fn set_task_tags(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
        tag_ids: Vec<Uuid>,
    ) -> Result<Vec<Tag>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = SetTaskTagsRequest { tag_ids };

        let response = self
            .client
            .put(&self.url(&format!(
                "/workspaces/{}/tasks/{}/tags",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn get_task_tags(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<Tag>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!(
                "/workspaces/{}/tasks/{}/tags",
                workspace_id, task_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    // ============ Documents ============

    pub async fn list_documents(&self, workspace_id: Uuid) -> Result<Vec<Document>, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!("/workspaces/{}/documents", workspace_id)))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn get_document(
        &self,
        workspace_id: Uuid,
        doc_id: Uuid,
    ) -> Result<Document, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .get(&self.url(&format!(
                "/workspaces/{}/documents/{}",
                workspace_id, doc_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn create_document(
        &self,
        workspace_id: Uuid,
        req: CreateDocumentRequest,
    ) -> Result<Document, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .post(&self.url(&format!("/workspaces/{}/documents", workspace_id)))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn update_document(
        &self,
        workspace_id: Uuid,
        doc_id: Uuid,
        req: UpdateDocumentRequest,
    ) -> Result<Document, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .patch(&self.url(&format!(
                "/workspaces/{}/documents/{}",
                workspace_id, doc_id
            )))
            .header("Authorization", &auth)
            .json(&req)
            .send()
            .await?;

        self.handle_response(response).await
    }

    pub async fn delete_document(&self, workspace_id: Uuid, doc_id: Uuid) -> Result<(), ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let response = self
            .client
            .delete(&self.url(&format!(
                "/workspaces/{}/documents/{}",
                workspace_id, doc_id
            )))
            .header("Authorization", &auth)
            .send()
            .await?;

        self.handle_empty_response(response).await
    }
}
