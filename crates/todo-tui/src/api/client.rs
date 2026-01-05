use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{Client, StatusCode};
use todo_shared::{
    api::{
        AuthResponse, CreateCommentRequest, CreateDocumentRequest, CreateStatusRequest,
        CreateTagRequest, CreateTaskRequest, CreateWorkspaceRequest, InviteDetails, LinkTaskRequest,
        LinkedDocument, LinkedTask, LoginRequest, MoveTaskRequest, RefreshRequest, RegisterRequest,
        RegisterResponse, ResendVerificationRequest, SearchResponse, SetTaskTagsRequest,
        TaskListParams, UpdateCommentRequest, UpdateDocumentRequest, UpdateStatusRequest,
        UpdateTagRequest, UpdateTaskRequest, UpdateWorkspaceRequest, VerifyEmailRequest,
        WorkspaceInvite, WorkspaceMemberWithUser, WorkspaceStats,
    },
    CommentWithAuthor, Document, Tag, Task, TaskStatus, User, Workspace, WorkspaceRole,
    WorkspaceSettings, WorkspaceWithRole,
};
use uuid::Uuid;

use super::auth::AuthTokens;

/// JWT payload claims we need for expiry checking
#[derive(serde::Deserialize)]
struct JwtClaims {
    exp: i64,
}

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

    /// Decode JWT payload and extract expiration time
    fn decode_token_exp(token: &str) -> Option<i64> {
        // JWT format: header.payload.signature
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return None;
        }

        // Decode base64 payload
        let payload = URL_SAFE_NO_PAD.decode(parts[1]).ok()?;
        let claims: JwtClaims = serde_json::from_slice(&payload).ok()?;

        Some(claims.exp)
    }

    /// Check if the access token is expiring soon (within 60 seconds)
    fn is_token_expiring_soon(&self) -> bool {
        let Some(tokens) = &self.tokens else {
            return true; // No token = treat as expired
        };

        let Some(exp) = Self::decode_token_exp(&tokens.access_token) else {
            return false; // Can't decode = don't refresh proactively
        };

        let now = chrono::Utc::now().timestamp();
        exp < now + 60 // Token expires within 60 seconds
    }

    /// Ensure we have a valid token, refreshing if needed
    /// Returns true if we have a valid token, false if refresh failed
    pub async fn ensure_valid_token(&mut self) -> bool {
        if !self.is_authenticated() {
            return false;
        }

        if self.is_token_expiring_soon() {
            // Try to refresh the token
            if self.refresh().await.is_err() {
                return false;
            }
        }

        true
    }

    // ============ Authenticated Request Helpers ============

    /// Make an authenticated GET request, auto-refreshing token if needed
    async fn authed_get(&mut self, path: &str) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .get(&self.url(path))
            .header("Authorization", self.auth_header().unwrap())
            .send()
            .await
            .map_err(ApiError::Network)
    }

    /// Make an authenticated POST request, auto-refreshing token if needed
    async fn authed_post<T: serde::Serialize>(
        &mut self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .post(&self.url(path))
            .header("Authorization", self.auth_header().unwrap())
            .json(body)
            .send()
            .await
            .map_err(ApiError::Network)
    }

    /// Make an authenticated POST request without body, auto-refreshing token if needed
    async fn authed_post_empty(&mut self, path: &str) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .post(&self.url(path))
            .header("Authorization", self.auth_header().unwrap())
            .send()
            .await
            .map_err(ApiError::Network)
    }

    /// Make an authenticated PATCH request, auto-refreshing token if needed
    async fn authed_patch<T: serde::Serialize>(
        &mut self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .patch(&self.url(path))
            .header("Authorization", self.auth_header().unwrap())
            .json(body)
            .send()
            .await
            .map_err(ApiError::Network)
    }

    /// Make an authenticated PUT request, auto-refreshing token if needed
    async fn authed_put<T: serde::Serialize>(
        &mut self,
        path: &str,
        body: &T,
    ) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .put(&self.url(path))
            .header("Authorization", self.auth_header().unwrap())
            .json(body)
            .send()
            .await
            .map_err(ApiError::Network)
    }

    /// Make an authenticated DELETE request, auto-refreshing token if needed
    async fn authed_delete(&mut self, path: &str) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .delete(&self.url(path))
            .header("Authorization", self.auth_header().unwrap())
            .send()
            .await
            .map_err(ApiError::Network)
    }

    /// Make an authenticated GET request to a full URL (for custom query params)
    async fn authed_get_url(&mut self, url: &str) -> Result<reqwest::Response, ApiError> {
        if !self.ensure_valid_token().await {
            return Err(ApiError::Unauthorized);
        }
        self.client
            .get(url)
            .header("Authorization", self.auth_header().unwrap())
            .send()
            .await
            .map_err(ApiError::Network)
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

    pub async fn me(&mut self) -> Result<User, ApiError> {
        let response = self.authed_get("/auth/me").await?;
        self.handle_response(response).await
    }

    // ============ Workspaces ============

    pub async fn list_workspaces(&mut self) -> Result<Vec<WorkspaceWithRole>, ApiError> {
        let response = self.authed_get("/workspaces").await?;
        self.handle_response(response).await
    }

    pub async fn create_workspace(
        &mut self,
        name: &str,
        description: Option<&str>,
    ) -> Result<Workspace, ApiError> {
        let req = CreateWorkspaceRequest {
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
        };
        let response = self.authed_post("/workspaces", &req).await?;
        self.handle_response(response).await
    }

    pub async fn get_workspace(&mut self, id: Uuid) -> Result<WorkspaceWithRole, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}", id)).await?;
        self.handle_response(response).await
    }

    pub async fn update_workspace(
        &mut self,
        id: Uuid,
        name: Option<&str>,
        description: Option<&str>,
        settings: Option<WorkspaceSettings>,
    ) -> Result<Workspace, ApiError> {
        let req = UpdateWorkspaceRequest {
            name: name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
            settings,
        };
        let response = self.authed_patch(&format!("/workspaces/{}", id), &req).await?;
        self.handle_response(response).await
    }

    pub async fn list_members(
        &mut self,
        workspace_id: Uuid,
    ) -> Result<Vec<WorkspaceMemberWithUser>, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}/members", workspace_id)).await?;
        self.handle_response(response).await
    }

    pub async fn delete_workspace(&mut self, id: Uuid) -> Result<(), ApiError> {
        let response = self.authed_delete(&format!("/workspaces/{}", id)).await?;
        self.handle_empty_response(response).await
    }

    pub async fn get_workspace_stats(&mut self, workspace_id: Uuid) -> Result<WorkspaceStats, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}/stats", workspace_id)).await?;
        self.handle_response(response).await
    }

    // ============ Member Management ============

    pub async fn create_invite(
        &mut self,
        workspace_id: Uuid,
        email: &str,
        role: WorkspaceRole,
    ) -> Result<WorkspaceInvite, ApiError> {
        let body = serde_json::json!({
            "email": email,
            "role": role
        });
        let response = self.authed_post(&format!("/workspaces/{}/invites", workspace_id), &body).await?;
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

    pub async fn accept_invite(&mut self, token: &str) -> Result<WorkspaceWithRole, ApiError> {
        let response = self.authed_post_empty(&format!("/invites/{}/accept", token)).await?;
        self.handle_response(response).await
    }

    pub async fn update_member_role(
        &mut self,
        workspace_id: Uuid,
        user_id: Uuid,
        role: WorkspaceRole,
    ) -> Result<WorkspaceMemberWithUser, ApiError> {
        let body = serde_json::json!({ "role": role });
        let response = self.authed_put(
            &format!("/workspaces/{}/members/{}", workspace_id, user_id),
            &body,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn remove_member(&mut self, workspace_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/members/{}", workspace_id, user_id),
        ).await?;
        self.handle_empty_response(response).await
    }

    // ============ Statuses ============

    pub async fn list_statuses(&mut self, workspace_id: Uuid) -> Result<Vec<TaskStatus>, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}/statuses", workspace_id)).await?;
        self.handle_response(response).await
    }

    pub async fn create_status(
        &mut self,
        workspace_id: Uuid,
        name: &str,
        color: Option<&str>,
        is_done: bool,
    ) -> Result<TaskStatus, ApiError> {
        let req = CreateStatusRequest {
            name: name.to_string(),
            color: color.map(|s| s.to_string()),
            is_done,
        };
        let response = self.authed_post(&format!("/workspaces/{}/statuses", workspace_id), &req).await?;
        self.handle_response(response).await
    }

    pub async fn update_status(
        &mut self,
        workspace_id: Uuid,
        status_id: Uuid,
        name: Option<&str>,
        color: Option<&str>,
        is_done: Option<bool>,
    ) -> Result<TaskStatus, ApiError> {
        let req = UpdateStatusRequest {
            name: name.map(|s| s.to_string()),
            color: color.map(|s| s.to_string()),
            is_done,
        };
        let response = self.authed_patch(
            &format!("/workspaces/{}/statuses/{}", workspace_id, status_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn delete_status(
        &mut self,
        workspace_id: Uuid,
        status_id: Uuid,
    ) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/statuses/{}", workspace_id, status_id),
        ).await?;
        self.handle_empty_response(response).await
    }

    pub async fn reorder_statuses(
        &mut self,
        workspace_id: Uuid,
        status_ids: Vec<Uuid>,
    ) -> Result<Vec<TaskStatus>, ApiError> {
        #[derive(serde::Serialize)]
        struct ReorderRequest {
            status_ids: Vec<Uuid>,
        }
        let req = ReorderRequest { status_ids };
        let response = self.authed_post(
            &format!("/workspaces/{}/statuses/reorder", workspace_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    // ============ Tasks ============

    pub async fn list_tasks(
        &mut self,
        workspace_id: Uuid,
        params: Option<&TaskListParams>,
    ) -> Result<TaskListResponse, ApiError> {
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

        let response = self.authed_get_url(&url).await?;
        self.handle_response(response).await
    }

    pub async fn create_task(
        &mut self,
        workspace_id: Uuid,
        req: CreateTaskRequest,
    ) -> Result<Task, ApiError> {
        let response = self.authed_post(&format!("/workspaces/{}/tasks", workspace_id), &req).await?;
        self.handle_response(response).await
    }

    pub async fn get_task(&mut self, workspace_id: Uuid, task_id: Uuid) -> Result<Task, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}/tasks/{}", workspace_id, task_id)).await?;
        self.handle_response(response).await
    }

    pub async fn update_task(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
        req: UpdateTaskRequest,
    ) -> Result<Task, ApiError> {
        let response = self.authed_patch(
            &format!("/workspaces/{}/tasks/{}", workspace_id, task_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn delete_task(&mut self, workspace_id: Uuid, task_id: Uuid) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/tasks/{}", workspace_id, task_id),
        ).await?;
        self.handle_empty_response(response).await
    }

    pub async fn move_task(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
        status_id: Uuid,
        position: Option<i32>,
    ) -> Result<Task, ApiError> {
        let req = MoveTaskRequest {
            status_id,
            position,
        };
        let response = self.authed_post(
            &format!("/workspaces/{}/tasks/{}/move", workspace_id, task_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    // ============ Search ============

    pub async fn search(
        &mut self,
        workspace_id: Uuid,
        query: &str,
        fuzzy: bool,
        page: Option<u32>,
        limit: Option<u32>,
    ) -> Result<SearchResponse, ApiError> {
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

        let response = self.authed_get_url(&url).await?;
        self.handle_response(response).await
    }

    // ============ Comments ============

    pub async fn list_comments(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<CommentWithAuthor>, ApiError> {
        let response = self.authed_get(
            &format!("/workspaces/{}/tasks/{}/comments", workspace_id, task_id),
        ).await?;
        self.handle_response(response).await
    }

    pub async fn create_comment(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
        content: &str,
    ) -> Result<CommentWithAuthor, ApiError> {
        let req = CreateCommentRequest {
            content: content.to_string(),
        };
        let response = self.authed_post(
            &format!("/workspaces/{}/tasks/{}/comments", workspace_id, task_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn update_comment(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
        comment_id: Uuid,
        content: &str,
    ) -> Result<CommentWithAuthor, ApiError> {
        let req = UpdateCommentRequest {
            content: content.to_string(),
        };
        let response = self.authed_patch(
            &format!("/workspaces/{}/tasks/{}/comments/{}", workspace_id, task_id, comment_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn delete_comment(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
        comment_id: Uuid,
    ) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/tasks/{}/comments/{}", workspace_id, task_id, comment_id),
        ).await?;
        self.handle_empty_response(response).await
    }

    // ============ Tags ============

    pub async fn list_tags(&mut self, workspace_id: Uuid) -> Result<Vec<Tag>, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}/tags", workspace_id)).await?;
        self.handle_response(response).await
    }

    pub async fn create_tag(
        &mut self,
        workspace_id: Uuid,
        name: &str,
        color: Option<&str>,
    ) -> Result<Tag, ApiError> {
        let req = CreateTagRequest {
            name: name.to_string(),
            color: color.map(|c| c.to_string()),
        };
        let response = self.authed_post(&format!("/workspaces/{}/tags", workspace_id), &req).await?;
        self.handle_response(response).await
    }

    pub async fn update_tag(
        &mut self,
        workspace_id: Uuid,
        tag_id: Uuid,
        name: Option<&str>,
        color: Option<&str>,
    ) -> Result<Tag, ApiError> {
        let req = UpdateTagRequest {
            name: name.map(|n| n.to_string()),
            color: color.map(|c| c.to_string()),
        };
        let response = self.authed_patch(
            &format!("/workspaces/{}/tags/{}", workspace_id, tag_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn delete_tag(&mut self, workspace_id: Uuid, tag_id: Uuid) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/tags/{}", workspace_id, tag_id),
        ).await?;
        self.handle_empty_response(response).await
    }

    pub async fn set_task_tags(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
        tag_ids: Vec<Uuid>,
    ) -> Result<Vec<Tag>, ApiError> {
        let req = SetTaskTagsRequest { tag_ids };
        let response = self.authed_put(
            &format!("/workspaces/{}/tasks/{}/tags", workspace_id, task_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn get_task_tags(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<Tag>, ApiError> {
        let response = self.authed_get(
            &format!("/workspaces/{}/tasks/{}/tags", workspace_id, task_id),
        ).await?;
        self.handle_response(response).await
    }

    // ============ Documents ============

    pub async fn list_documents(&mut self, workspace_id: Uuid) -> Result<Vec<Document>, ApiError> {
        let response = self.authed_get(&format!("/workspaces/{}/documents", workspace_id)).await?;
        self.handle_response(response).await
    }

    pub async fn get_document(
        &mut self,
        workspace_id: Uuid,
        doc_id: Uuid,
    ) -> Result<Document, ApiError> {
        let response = self.authed_get(
            &format!("/workspaces/{}/documents/{}", workspace_id, doc_id),
        ).await?;
        self.handle_response(response).await
    }

    pub async fn create_document(
        &mut self,
        workspace_id: Uuid,
        req: CreateDocumentRequest,
    ) -> Result<Document, ApiError> {
        let response = self.authed_post(
            &format!("/workspaces/{}/documents", workspace_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn update_document(
        &mut self,
        workspace_id: Uuid,
        doc_id: Uuid,
        req: UpdateDocumentRequest,
    ) -> Result<Document, ApiError> {
        let response = self.authed_patch(
            &format!("/workspaces/{}/documents/{}", workspace_id, doc_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn delete_document(&mut self, workspace_id: Uuid, doc_id: Uuid) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/documents/{}", workspace_id, doc_id),
        ).await?;
        self.handle_empty_response(response).await
    }

    // ============ Task-Document Links ============

    pub async fn list_linked_documents(
        &mut self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<LinkedDocument>, ApiError> {
        let response = self.authed_get(
            &format!("/workspaces/{}/tasks/{}/documents", workspace_id, task_id),
        ).await?;
        self.handle_response(response).await
    }

    pub async fn list_linked_tasks(
        &mut self,
        workspace_id: Uuid,
        doc_id: Uuid,
    ) -> Result<Vec<LinkedTask>, ApiError> {
        let response = self.authed_get(
            &format!("/workspaces/{}/documents/{}/tasks", workspace_id, doc_id),
        ).await?;
        self.handle_response(response).await
    }

    pub async fn link_task_to_document(
        &mut self,
        workspace_id: Uuid,
        doc_id: Uuid,
        task_id: Uuid,
    ) -> Result<LinkedTask, ApiError> {
        let req = LinkTaskRequest { task_id };
        let response = self.authed_post(
            &format!("/workspaces/{}/documents/{}/tasks", workspace_id, doc_id),
            &req,
        ).await?;
        self.handle_response(response).await
    }

    pub async fn unlink_task_from_document(
        &mut self,
        workspace_id: Uuid,
        doc_id: Uuid,
        task_id: Uuid,
    ) -> Result<(), ApiError> {
        let response = self.authed_delete(
            &format!("/workspaces/{}/documents/{}/tasks/{}", workspace_id, doc_id, task_id),
        ).await?;
        self.handle_empty_response(response).await
    }
}
