use anyhow::Result;
use reqwest::{Client, StatusCode};
use todo_shared::{
    api::{
        AuthResponse, CreateTaskRequest, CreateWorkspaceRequest, LoginRequest,
        MoveTaskRequest, RefreshRequest, RegisterRequest, UpdateTaskRequest,
        UpdateWorkspaceRequest, CreateStatusRequest, UpdateStatusRequest,
        CreateCommentRequest, UpdateCommentRequest,
    },
    Comment, Task, TaskStatus, User, Workspace, WorkspaceWithRole,
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
            StatusCode::FORBIDDEN => Err(ApiError::Forbidden),
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
            StatusCode::FORBIDDEN => Err(ApiError::Forbidden),
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
        email: &str,
        password: &str,
        display_name: &str,
    ) -> Result<User, ApiError> {
        let req = RegisterRequest {
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

        let auth: AuthResponse = self.handle_response(response).await?;

        // Store tokens (same as login)
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
    ) -> Result<Workspace, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let req = UpdateWorkspaceRequest {
            name: name.map(|s| s.to_string()),
            description: description.map(|s| s.to_string()),
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
        status_id: Option<Uuid>,
    ) -> Result<TaskListResponse, ApiError> {
        let auth = self.auth_header().ok_or(ApiError::Unauthorized)?;

        let mut url = self.url(&format!("/workspaces/{}/tasks", workspace_id));
        if let Some(status_id) = status_id {
            url.push_str(&format!("?status_id={}", status_id));
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

    // ============ Comments ============

    pub async fn list_comments(
        &self,
        workspace_id: Uuid,
        task_id: Uuid,
    ) -> Result<Vec<Comment>, ApiError> {
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
    ) -> Result<Comment, ApiError> {
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
    ) -> Result<Comment, ApiError> {
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
}
