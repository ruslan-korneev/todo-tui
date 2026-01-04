use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use todo_shared::{Task, TaskStatus, Workspace, WorkspaceWithRole, User};
use tokio::sync::mpsc;

use crate::api::ApiClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    Login,
    VerifyingAuth,
    WorkspaceSelect,
    Dashboard,
    TaskDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Normal,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Email,
    Password,
}

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    VerifyAuth,
    AuthSuccess,
    AuthFailed(String),
    WorkspacesLoaded(Vec<WorkspaceWithRole>),
    WorkspaceDataLoaded {
        statuses: Vec<TaskStatus>,
        tasks: Vec<Task>,
    },
    Error(String),
}

pub struct App {
    pub api: ApiClient,
    pub view: View,
    pub vim_mode: VimMode,

    // Loading state
    pub loading: bool,
    pub loading_message: String,
    pub error_message: Option<String>,

    // Current user
    pub user: Option<User>,

    // Login form
    pub login_email: String,
    pub login_password: String,
    pub login_field: InputField,

    // Workspace selection
    pub workspaces: Vec<WorkspaceWithRole>,
    pub selected_workspace_idx: usize,

    // Dashboard state
    pub current_workspace: Option<Workspace>,
    pub columns: Vec<Column>,
    pub selected_column: usize,
    pub selected_task: usize,
}

pub struct Column {
    pub status: TaskStatus,
    pub tasks: Vec<Task>,
}

impl App {
    pub fn new(api: ApiClient, has_tokens: bool) -> Self {
        let view = if has_tokens {
            View::VerifyingAuth
        } else {
            View::Login
        };

        Self {
            api,
            view,
            vim_mode: VimMode::Normal,
            loading: false,
            loading_message: String::new(),
            error_message: None,
            user: None,
            login_email: String::new(),
            login_password: String::new(),
            login_field: InputField::Email,
            workspaces: Vec::new(),
            selected_workspace_idx: 0,
            current_workspace: None,
            columns: Vec::new(),
            selected_column: 0,
            selected_task: 0,
        }
    }

    pub fn set_loading(&mut self, loading: bool, message: &str) {
        self.loading = loading;
        self.loading_message = message.to_string();
    }

    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    pub fn clear_error(&mut self) {
        self.error_message = None;
    }

    /// Handle key events, returns true if app should quit
    pub async fn handle_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        // Clear error on any key press
        if self.error_message.is_some() && key.code != KeyCode::Esc {
            self.clear_error();
        }

        // Global quit with Ctrl+C
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Ok(true);
        }

        match self.view {
            View::Login => self.handle_login_key(key, tx).await,
            View::VerifyingAuth => Ok(false), // No input during verification
            View::WorkspaceSelect => self.handle_workspace_select_key(key, tx).await,
            View::Dashboard => self.handle_dashboard_key(key, tx).await,
            View::TaskDetail => Ok(false), // TODO
        }
    }

    async fn handle_login_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        if self.loading {
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') if self.vim_mode == VimMode::Normal => return Ok(true),
            KeyCode::Esc => {
                if self.vim_mode == VimMode::Insert {
                    self.vim_mode = VimMode::Normal;
                }
            }
            KeyCode::Char('i') if self.vim_mode == VimMode::Normal => {
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.login_field = match self.login_field {
                    InputField::Email => InputField::Password,
                    InputField::Password => InputField::Email,
                };
            }
            KeyCode::Char('j') | KeyCode::Down if self.vim_mode == VimMode::Normal => {
                self.login_field = InputField::Password;
            }
            KeyCode::Char('k') | KeyCode::Up if self.vim_mode == VimMode::Normal => {
                self.login_field = InputField::Email;
            }
            KeyCode::Enter => {
                if !self.login_email.is_empty() && !self.login_password.is_empty() {
                    self.do_login(tx).await;
                }
            }
            KeyCode::Char(c) if self.vim_mode == VimMode::Insert => {
                match self.login_field {
                    InputField::Email => self.login_email.push(c),
                    InputField::Password => self.login_password.push(c),
                }
            }
            KeyCode::Backspace if self.vim_mode == VimMode::Insert => {
                match self.login_field {
                    InputField::Email => { self.login_email.pop(); }
                    InputField::Password => { self.login_password.pop(); }
                }
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_workspace_select_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        if self.loading {
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected_workspace_idx < self.workspaces.len().saturating_sub(1) {
                    self.selected_workspace_idx += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.selected_workspace_idx > 0 {
                    self.selected_workspace_idx -= 1;
                }
            }
            KeyCode::Enter => {
                if let Some(ws) = self.workspaces.get(self.selected_workspace_idx) {
                    self.current_workspace = Some(ws.workspace.clone());
                    self.load_workspace_data(tx).await;
                }
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_dashboard_key(
        &mut self,
        key: KeyEvent,
        _tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('h') | KeyCode::Left => self.move_left(),
            KeyCode::Char('l') | KeyCode::Right => self.move_right(),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            _ => {}
        }

        Ok(false)
    }

    async fn do_login(&mut self, tx: mpsc::Sender<AppEvent>) {
        self.set_loading(true, "Logging in...");

        let email = self.login_email.clone();
        let password = self.login_password.clone();

        match self.api.login(&email, &password).await {
            Ok(user) => {
                self.user = Some(user);
                let _ = tx.send(AppEvent::AuthSuccess).await;
            }
            Err(e) => {
                let _ = tx.send(AppEvent::AuthFailed(e.to_string())).await;
            }
        }

        self.set_loading(false, "");
    }

    pub async fn verify_auth(&mut self) {
        self.set_loading(true, "Verifying authentication...");

        match self.api.me().await {
            Ok(user) => {
                self.user = Some(user);
                self.view = View::WorkspaceSelect;
                self.load_workspaces().await;
            }
            Err(_) => {
                // Token invalid, go to login
                let _ = self.api.logout().await;
                self.view = View::Login;
            }
        }

        self.set_loading(false, "");
    }

    pub async fn on_auth_success(&mut self) {
        self.view = View::WorkspaceSelect;
        self.login_password.clear();
        self.load_workspaces().await;
    }

    pub fn on_auth_failed(&mut self, msg: String) {
        self.set_error(format!("Login failed: {}", msg));
        self.login_password.clear();
    }

    async fn load_workspaces(&mut self) {
        self.set_loading(true, "Loading workspaces...");

        match self.api.list_workspaces().await {
            Ok(workspaces) => {
                self.workspaces = workspaces;
            }
            Err(e) => {
                self.set_error(format!("Failed to load workspaces: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    pub fn on_workspaces_loaded(&mut self, workspaces: Vec<WorkspaceWithRole>) {
        self.workspaces = workspaces;
        self.set_loading(false, "");
    }

    async fn load_workspace_data(&mut self, _tx: mpsc::Sender<AppEvent>) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        self.set_loading(true, "Loading workspace data...");

        // Load statuses
        let statuses = match self.api.list_statuses(workspace_id).await {
            Ok(s) => s,
            Err(e) => {
                self.set_error(format!("Failed to load statuses: {}", e));
                self.set_loading(false, "");
                return;
            }
        };

        // Load tasks
        let tasks = match self.api.list_tasks(workspace_id, None).await {
            Ok(response) => response.tasks,
            Err(e) => {
                self.set_error(format!("Failed to load tasks: {}", e));
                self.set_loading(false, "");
                return;
            }
        };

        self.on_workspace_data_loaded(statuses, tasks);
    }

    pub fn on_workspace_data_loaded(&mut self, statuses: Vec<TaskStatus>, tasks: Vec<Task>) {
        // Organize tasks into columns
        self.columns = statuses
            .into_iter()
            .map(|status| {
                let column_tasks: Vec<Task> = tasks
                    .iter()
                    .filter(|t| t.status_id == status.id)
                    .cloned()
                    .collect();
                Column {
                    status,
                    tasks: column_tasks,
                }
            })
            .collect();

        self.selected_column = 0;
        self.selected_task = 0;
        self.view = View::Dashboard;
        self.set_loading(false, "");
    }

    pub fn move_left(&mut self) {
        if self.selected_column > 0 {
            self.selected_column -= 1;
            self.selected_task = 0;
        }
    }

    pub fn move_right(&mut self) {
        if !self.columns.is_empty() && self.selected_column < self.columns.len() - 1 {
            self.selected_column += 1;
            self.selected_task = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_task > 0 {
            self.selected_task -= 1;
        }
    }

    pub fn move_down(&mut self) {
        if let Some(column) = self.columns.get(self.selected_column) {
            if self.selected_task < column.tasks.len().saturating_sub(1) {
                self.selected_task += 1;
            }
        }
    }
}
