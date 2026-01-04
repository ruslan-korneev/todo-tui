use anyhow::Result;
use chrono::NaiveDate;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use todo_shared::{Comment, Priority, Task, TaskStatus, Workspace, WorkspaceWithRole, User};
use todo_shared::api::{CreateTaskRequest, UpdateTaskRequest};
use tokio::sync::mpsc;

use crate::api::ApiClient;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Views for future implementation
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
pub enum AuthMode {
    Login,
    Register,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputField {
    Email,
    Password,
    DisplayName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewTaskField {
    Title,
    Description,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskEditField {
    Title,
    Description,
    Priority,
    DueDate,
    TimeEstimate,
}

#[derive(Debug)]
#[allow(dead_code)] // Event variants for future async operations
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

    // Login/Register form
    pub auth_mode: AuthMode,
    pub login_email: String,
    pub login_password: String,
    pub login_field: InputField,
    pub register_display_name: String,

    // Workspace selection
    pub workspaces: Vec<WorkspaceWithRole>,
    pub selected_workspace_idx: usize,
    pub creating_workspace: bool,
    pub new_workspace_name: String,

    // Dashboard state
    pub current_workspace: Option<Workspace>,
    pub columns: Vec<Column>,
    pub selected_column: usize,
    pub selected_task: usize,
    pub moving_task: bool,
    #[allow(dead_code)] // Prepared for scroll feature
    pub column_scroll_offsets: Vec<usize>,

    // Task detail state
    pub selected_task_detail: Option<Task>,
    pub task_comments: Vec<Comment>,
    pub adding_comment: bool,
    pub new_comment_content: String,

    // Create task state
    pub creating_task: bool,
    pub new_task_title: String,
    pub new_task_description: String,
    pub new_task_field: NewTaskField,

    // Delete task state
    pub confirming_delete: bool,

    // Edit task state
    pub editing_task: bool,
    pub edit_field: TaskEditField,
    pub edit_task_title: String,
    pub edit_task_description: String,
    pub edit_task_priority: Option<Priority>,
    pub edit_task_due_date_str: String,
    pub edit_task_time_estimate_str: String,
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
            auth_mode: AuthMode::Login,
            login_email: String::new(),
            login_password: String::new(),
            login_field: InputField::Email,
            register_display_name: String::new(),
            workspaces: Vec::new(),
            selected_workspace_idx: 0,
            creating_workspace: false,
            new_workspace_name: String::new(),
            current_workspace: None,
            columns: Vec::new(),
            selected_column: 0,
            selected_task: 0,
            moving_task: false,
            column_scroll_offsets: Vec::new(),
            selected_task_detail: None,
            task_comments: Vec::new(),
            adding_comment: false,
            new_comment_content: String::new(),
            creating_task: false,
            new_task_title: String::new(),
            new_task_description: String::new(),
            new_task_field: NewTaskField::Title,
            confirming_delete: false,
            editing_task: false,
            edit_field: TaskEditField::Title,
            edit_task_title: String::new(),
            edit_task_description: String::new(),
            edit_task_priority: None,
            edit_task_due_date_str: String::new(),
            edit_task_time_estimate_str: String::new(),
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
            View::TaskDetail => self.handle_task_detail_key(key, tx).await,
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
            // Toggle between Login and Register modes
            KeyCode::Char('r') if self.vim_mode == VimMode::Normal => {
                self.auth_mode = AuthMode::Register;
                self.login_field = InputField::Email;
            }
            KeyCode::Char('l') if self.vim_mode == VimMode::Normal => {
                self.auth_mode = AuthMode::Login;
                self.login_field = InputField::Email;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.login_field = match (self.auth_mode, self.login_field) {
                    (AuthMode::Login, InputField::Email) => InputField::Password,
                    (AuthMode::Login, InputField::Password) => InputField::Email,
                    (AuthMode::Login, InputField::DisplayName) => InputField::Email,
                    (AuthMode::Register, InputField::Email) => InputField::Password,
                    (AuthMode::Register, InputField::Password) => InputField::DisplayName,
                    (AuthMode::Register, InputField::DisplayName) => InputField::Email,
                };
            }
            KeyCode::Char('j') | KeyCode::Down if self.vim_mode == VimMode::Normal => {
                self.login_field = match (self.auth_mode, self.login_field) {
                    (AuthMode::Login, InputField::Email) => InputField::Password,
                    (AuthMode::Register, InputField::Email) => InputField::Password,
                    (AuthMode::Register, InputField::Password) => InputField::DisplayName,
                    _ => self.login_field,
                };
            }
            KeyCode::Char('k') | KeyCode::Up if self.vim_mode == VimMode::Normal => {
                self.login_field = match (self.auth_mode, self.login_field) {
                    (AuthMode::Login, InputField::Password) => InputField::Email,
                    (AuthMode::Register, InputField::Password) => InputField::Email,
                    (AuthMode::Register, InputField::DisplayName) => InputField::Password,
                    _ => self.login_field,
                };
            }
            KeyCode::Enter => {
                match self.auth_mode {
                    AuthMode::Login => {
                        if !self.login_email.is_empty() && !self.login_password.is_empty() {
                            self.do_login(tx).await;
                        }
                    }
                    AuthMode::Register => {
                        if !self.login_email.is_empty()
                            && !self.login_password.is_empty()
                            && !self.register_display_name.is_empty()
                        {
                            self.do_register(tx).await;
                        }
                    }
                }
            }
            KeyCode::Char(c) if self.vim_mode == VimMode::Insert => {
                match self.login_field {
                    InputField::Email => self.login_email.push(c),
                    InputField::Password => self.login_password.push(c),
                    InputField::DisplayName => self.register_display_name.push(c),
                }
            }
            KeyCode::Backspace if self.vim_mode == VimMode::Insert => {
                match self.login_field {
                    InputField::Email => { self.login_email.pop(); }
                    InputField::Password => { self.login_password.pop(); }
                    InputField::DisplayName => { self.register_display_name.pop(); }
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

        // Handle workspace creation mode
        if self.creating_workspace {
            match key.code {
                KeyCode::Esc => {
                    self.creating_workspace = false;
                    self.new_workspace_name.clear();
                }
                KeyCode::Enter => {
                    if !self.new_workspace_name.is_empty() {
                        self.do_create_workspace().await;
                    }
                }
                KeyCode::Char(c) => {
                    self.new_workspace_name.push(c);
                }
                KeyCode::Backspace => {
                    self.new_workspace_name.pop();
                }
                _ => {}
            }
            return Ok(false);
        }

        // Normal workspace selection mode
        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('L') => {
                self.do_logout().await;
            }
            KeyCode::Char('n') => {
                self.creating_workspace = true;
                self.new_workspace_name.clear();
            }
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

    async fn do_create_workspace(&mut self) {
        self.set_loading(true, "Creating workspace...");

        let name = self.new_workspace_name.clone();

        match self.api.create_workspace(&name, None).await {
            Ok(_) => {
                self.creating_workspace = false;
                self.new_workspace_name.clear();
                self.load_workspaces().await;
            }
            Err(e) => {
                self.set_error(format!("Failed to create workspace: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    async fn do_logout(&mut self) {
        let _ = self.api.logout().await;
        self.user = None;
        self.workspaces.clear();
        self.current_workspace = None;
        self.columns.clear();
        self.view = View::Login;
    }

    async fn handle_dashboard_key(
        &mut self,
        key: KeyEvent,
        _tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        // Handle create task popup
        if self.creating_task {
            match key.code {
                KeyCode::Esc => {
                    self.creating_task = false;
                    self.new_task_title.clear();
                    self.new_task_description.clear();
                    self.new_task_field = NewTaskField::Title;
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.new_task_field = match self.new_task_field {
                        NewTaskField::Title => NewTaskField::Description,
                        NewTaskField::Description => NewTaskField::Title,
                    };
                }
                KeyCode::Enter => {
                    if !self.new_task_title.is_empty() {
                        self.do_create_task().await;
                    }
                }
                KeyCode::Char(c) => {
                    match self.new_task_field {
                        NewTaskField::Title => self.new_task_title.push(c),
                        NewTaskField::Description => self.new_task_description.push(c),
                    }
                }
                KeyCode::Backspace => {
                    match self.new_task_field {
                        NewTaskField::Title => { self.new_task_title.pop(); }
                        NewTaskField::Description => { self.new_task_description.pop(); }
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        // Handle delete confirmation
        if self.confirming_delete {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.do_delete_task().await;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirming_delete = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        // Handle move mode
        if self.moving_task {
            match key.code {
                KeyCode::Esc => {
                    self.moving_task = false;
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    self.do_move_task_left().await;
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    self.do_move_task_right().await;
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Backspace => self.go_back_to_workspace_select(),
            KeyCode::Char('h') | KeyCode::Left => self.move_left(),
            KeyCode::Char('l') | KeyCode::Right => self.move_right(),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('m') => {
                // Enter move mode if there's a selected task
                if self.get_selected_task().is_some() {
                    self.moving_task = true;
                }
            }
            KeyCode::Char('n') => {
                // Create new task
                if !self.columns.is_empty() {
                    self.creating_task = true;
                    self.new_task_field = NewTaskField::Title;
                    self.vim_mode = VimMode::Insert;
                }
            }
            KeyCode::Char('d') => {
                // Delete task
                if self.get_selected_task().is_some() {
                    self.confirming_delete = true;
                }
            }
            KeyCode::Enter => {
                self.open_task_detail().await;
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_task_detail_key(
        &mut self,
        key: KeyEvent,
        _tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        // Handle edit mode
        if self.editing_task {
            return self.handle_edit_task_key(key).await;
        }

        // Handle comment input mode
        if self.adding_comment {
            match key.code {
                KeyCode::Esc => {
                    self.adding_comment = false;
                    self.new_comment_content.clear();
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    if !self.new_comment_content.is_empty() {
                        self.do_add_comment().await;
                    }
                }
                KeyCode::Char(c) => {
                    self.new_comment_content.push(c);
                }
                KeyCode::Backspace => {
                    self.new_comment_content.pop();
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.close_task_detail();
            }
            KeyCode::Char('a') => {
                // Add comment
                self.adding_comment = true;
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('e') => {
                // Enter edit mode
                self.enter_edit_mode();
            }
            KeyCode::Char('j') | KeyCode::Down => {
                // Scroll comments down (future enhancement)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                // Scroll comments up (future enhancement)
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_edit_task_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Insert mode - editing current field
        if self.vim_mode == VimMode::Insert {
            match key.code {
                KeyCode::Esc => {
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    // Save and exit insert mode
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Char(c) => {
                    match self.edit_field {
                        TaskEditField::Title => self.edit_task_title.push(c),
                        TaskEditField::Description => self.edit_task_description.push(c),
                        TaskEditField::DueDate => self.edit_task_due_date_str.push(c),
                        TaskEditField::TimeEstimate => self.edit_task_time_estimate_str.push(c),
                        TaskEditField::Priority => {} // Priority uses h/l, not text input
                    }
                }
                KeyCode::Backspace => {
                    match self.edit_field {
                        TaskEditField::Title => { self.edit_task_title.pop(); }
                        TaskEditField::Description => { self.edit_task_description.pop(); }
                        TaskEditField::DueDate => { self.edit_task_due_date_str.pop(); }
                        TaskEditField::TimeEstimate => { self.edit_task_time_estimate_str.pop(); }
                        TaskEditField::Priority => {}
                    }
                }
                _ => {}
            }
            return Ok(false);
        }

        // Normal mode in edit
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                // Cancel edit mode
                self.editing_task = false;
                self.vim_mode = VimMode::Normal;
            }
            KeyCode::Char('i') => {
                // Enter insert mode for current field (except Priority)
                if self.edit_field != TaskEditField::Priority {
                    self.vim_mode = VimMode::Insert;
                }
            }
            KeyCode::Tab | KeyCode::Char('j') | KeyCode::Down => {
                // Next field
                self.edit_field = match self.edit_field {
                    TaskEditField::Title => TaskEditField::Description,
                    TaskEditField::Description => TaskEditField::Priority,
                    TaskEditField::Priority => TaskEditField::DueDate,
                    TaskEditField::DueDate => TaskEditField::TimeEstimate,
                    TaskEditField::TimeEstimate => TaskEditField::Title,
                };
            }
            KeyCode::BackTab | KeyCode::Char('k') | KeyCode::Up => {
                // Previous field
                self.edit_field = match self.edit_field {
                    TaskEditField::Title => TaskEditField::TimeEstimate,
                    TaskEditField::Description => TaskEditField::Title,
                    TaskEditField::Priority => TaskEditField::Description,
                    TaskEditField::DueDate => TaskEditField::Priority,
                    TaskEditField::TimeEstimate => TaskEditField::DueDate,
                };
            }
            KeyCode::Char('h') | KeyCode::Left if self.edit_field == TaskEditField::Priority => {
                // Decrease priority
                self.edit_task_priority = Some(match self.edit_task_priority {
                    Some(Priority::Highest) => Priority::High,
                    Some(Priority::High) => Priority::Medium,
                    Some(Priority::Medium) => Priority::Low,
                    Some(Priority::Low) => Priority::Lowest,
                    Some(Priority::Lowest) | None => Priority::Lowest,
                });
            }
            KeyCode::Char('l') | KeyCode::Right if self.edit_field == TaskEditField::Priority => {
                // Increase priority
                self.edit_task_priority = Some(match self.edit_task_priority {
                    Some(Priority::Lowest) => Priority::Low,
                    Some(Priority::Low) => Priority::Medium,
                    Some(Priority::Medium) => Priority::High,
                    Some(Priority::High) => Priority::Highest,
                    Some(Priority::Highest) | None => Priority::Highest,
                });
            }
            KeyCode::Enter => {
                // Save changes
                self.do_update_task().await;
            }
            _ => {}
        }

        Ok(false)
    }

    fn go_back_to_workspace_select(&mut self) {
        self.current_workspace = None;
        self.columns.clear();
        self.selected_column = 0;
        self.selected_task = 0;
        self.view = View::WorkspaceSelect;
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

    async fn do_register(&mut self, tx: mpsc::Sender<AppEvent>) {
        self.set_loading(true, "Registering...");

        let email = self.login_email.clone();
        let password = self.login_password.clone();
        let display_name = self.register_display_name.clone();

        match self.api.register(&email, &password, &display_name).await {
            Ok(user) => {
                self.user = Some(user);
                let _ = tx.send(AppEvent::AuthSuccess).await;
            }
            Err(e) => {
                self.set_error(format!("Registration failed: {}", e));
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

    pub fn get_selected_task(&self) -> Option<&Task> {
        self.columns
            .get(self.selected_column)
            .and_then(|col| col.tasks.get(self.selected_task))
    }

    async fn do_move_task_left(&mut self) {
        if self.selected_column == 0 {
            return;
        }

        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let task = match self.get_selected_task() {
            Some(t) => t.clone(),
            None => return,
        };

        let target_column = self.selected_column - 1;
        let target_status_id = self.columns[target_column].status.id;

        match self.api.move_task(workspace_id, task.id, target_status_id, None).await {
            Ok(updated_task) => {
                // Remove from current column
                if let Some(col) = self.columns.get_mut(self.selected_column) {
                    col.tasks.retain(|t| t.id != task.id);
                }
                // Add to target column
                if let Some(col) = self.columns.get_mut(target_column) {
                    col.tasks.push(updated_task);
                    col.tasks.sort_by_key(|t| t.position);
                }
                // Move selection
                self.selected_column = target_column;
                self.selected_task = self.columns[target_column].tasks.len().saturating_sub(1);
                self.moving_task = false;
            }
            Err(e) => {
                self.set_error(format!("Failed to move task: {}", e));
            }
        }
    }

    async fn do_move_task_right(&mut self) {
        if self.columns.is_empty() || self.selected_column >= self.columns.len() - 1 {
            return;
        }

        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let task = match self.get_selected_task() {
            Some(t) => t.clone(),
            None => return,
        };

        let target_column = self.selected_column + 1;
        let target_status_id = self.columns[target_column].status.id;

        match self.api.move_task(workspace_id, task.id, target_status_id, None).await {
            Ok(updated_task) => {
                // Remove from current column
                if let Some(col) = self.columns.get_mut(self.selected_column) {
                    col.tasks.retain(|t| t.id != task.id);
                }
                // Add to target column
                if let Some(col) = self.columns.get_mut(target_column) {
                    col.tasks.push(updated_task);
                    col.tasks.sort_by_key(|t| t.position);
                }
                // Move selection
                self.selected_column = target_column;
                self.selected_task = self.columns[target_column].tasks.len().saturating_sub(1);
                self.moving_task = false;
            }
            Err(e) => {
                self.set_error(format!("Failed to move task: {}", e));
            }
        }
    }

    async fn open_task_detail(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let task = match self.get_selected_task() {
            Some(t) => t.clone(),
            None => return,
        };

        self.set_loading(true, "Loading task details...");

        // Load comments
        match self.api.list_comments(workspace_id, task.id).await {
            Ok(comments) => {
                self.task_comments = comments;
            }
            Err(e) => {
                self.set_error(format!("Failed to load comments: {}", e));
                self.set_loading(false, "");
                return;
            }
        }

        self.selected_task_detail = Some(task);
        self.view = View::TaskDetail;
        self.set_loading(false, "");
    }

    fn close_task_detail(&mut self) {
        self.selected_task_detail = None;
        self.task_comments.clear();
        self.adding_comment = false;
        self.new_comment_content.clear();
        self.vim_mode = VimMode::Normal;
        self.view = View::Dashboard;
    }

    async fn do_add_comment(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let task_id = match self.selected_task_detail {
            Some(ref t) => t.id,
            None => return,
        };

        let content = self.new_comment_content.clone();

        match self.api.create_comment(workspace_id, task_id, &content).await {
            Ok(comment) => {
                self.task_comments.push(comment);
                self.new_comment_content.clear();
                self.adding_comment = false;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to add comment: {}", e));
            }
        }
    }

    async fn do_create_task(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        // Use currently selected column's status
        let status_id = match self.columns.get(self.selected_column) {
            Some(col) => col.status.id,
            None => return,
        };

        let req = CreateTaskRequest {
            title: self.new_task_title.clone(),
            status_id,
            description: if self.new_task_description.is_empty() {
                None
            } else {
                Some(self.new_task_description.clone())
            },
            priority: None,
            due_date: None,
            time_estimate_minutes: None,
            assigned_to: None,
        };

        self.set_loading(true, "Creating task...");

        match self.api.create_task(workspace_id, req).await {
            Ok(task) => {
                // Add to current column
                if let Some(col) = self.columns.get_mut(self.selected_column) {
                    col.tasks.push(task);
                    col.tasks.sort_by_key(|t| t.position);
                    // Select the new task
                    self.selected_task = col.tasks.len().saturating_sub(1);
                }
                // Clear form
                self.creating_task = false;
                self.new_task_title.clear();
                self.new_task_description.clear();
                self.new_task_field = NewTaskField::Title;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to create task: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    async fn do_delete_task(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let task = match self.get_selected_task() {
            Some(t) => t.clone(),
            None => return,
        };

        self.set_loading(true, "Deleting task...");

        match self.api.delete_task(workspace_id, task.id).await {
            Ok(()) => {
                // Remove from current column
                if let Some(col) = self.columns.get_mut(self.selected_column) {
                    col.tasks.retain(|t| t.id != task.id);
                    // Adjust selection if needed
                    if self.selected_task >= col.tasks.len() && !col.tasks.is_empty() {
                        self.selected_task = col.tasks.len() - 1;
                    }
                }
                self.confirming_delete = false;
            }
            Err(e) => {
                self.set_error(format!("Failed to delete task: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    fn enter_edit_mode(&mut self) {
        if let Some(ref task) = self.selected_task_detail {
            self.editing_task = true;
            self.edit_field = TaskEditField::Title;
            self.edit_task_title = task.title.clone();
            self.edit_task_description = task.description.clone().unwrap_or_default();
            self.edit_task_priority = task.priority;
            self.edit_task_due_date_str = task.due_date.map(|d| d.to_string()).unwrap_or_default();
            self.edit_task_time_estimate_str = task.time_estimate_minutes
                .map(|m| m.to_string())
                .unwrap_or_default();
        }
    }

    async fn do_update_task(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let task_id = match self.selected_task_detail {
            Some(ref t) => t.id,
            None => return,
        };

        // Parse due date
        let due_date = if self.edit_task_due_date_str.is_empty() {
            None
        } else {
            NaiveDate::parse_from_str(&self.edit_task_due_date_str, "%Y-%m-%d").ok()
        };

        // Parse time estimate
        let time_estimate_minutes = if self.edit_task_time_estimate_str.is_empty() {
            None
        } else {
            self.edit_task_time_estimate_str.parse::<i32>().ok()
        };

        let req = UpdateTaskRequest {
            title: Some(self.edit_task_title.clone()),
            status_id: None,
            description: Some(if self.edit_task_description.is_empty() {
                None
            } else {
                Some(self.edit_task_description.clone())
            }).flatten(),
            priority: self.edit_task_priority,
            due_date,
            time_estimate_minutes,
            assigned_to: None,
        };

        self.set_loading(true, "Updating task...");

        match self.api.update_task(workspace_id, task_id, req).await {
            Ok(updated_task) => {
                // Update the task detail
                self.selected_task_detail = Some(updated_task.clone());

                // Update in columns
                for col in &mut self.columns {
                    for task in &mut col.tasks {
                        if task.id == task_id {
                            *task = updated_task.clone();
                        }
                    }
                }

                self.editing_task = false;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to update task: {}", e));
            }
        }

        self.set_loading(false, "");
    }
}
