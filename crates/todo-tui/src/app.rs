use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashSet;
use todo_shared::api::{CreateDocumentRequest, CreateTaskRequest, SearchResultItem, TaskListParams, UpdateDocumentRequest, UpdateTaskRequest, WorkspaceMemberWithUser};
use todo_shared::{CommentWithAuthor, Document, Priority, Tag, Task, TaskStatus, User, Workspace, WorkspaceWithRole};
use tokio::sync::mpsc;
use tui_textarea::TextArea;

use crate::api::{ApiClient, UserPreferences, WorkspaceState};
use crate::editor::{self, EditorContext};

/// Preset colors for tags (hex format)
pub const TAG_COLORS: &[&str] = &[
    "#EF4444", // Red
    "#F97316", // Orange
    "#EAB308", // Yellow
    "#22C55E", // Green
    "#06B6D4", // Cyan
    "#3B82F6", // Blue
    "#8B5CF6", // Purple
    "#EC4899", // Pink
    "#6B7280", // Gray
];

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Views for future implementation
pub enum View {
    Login,
    EmailVerification,
    VerifyingAuth,
    WorkspaceSelect,
    Home,
    Dashboard,  // Kanban board
    TaskDetail,
    KnowledgeBase,
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
    Username,
    Email,
    Password,
    DisplayName,
    VerificationCode,
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
    Assignee,
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagManagementMode {
    List,
    Create,
    Edit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterPanelSection {
    #[default]
    Priority,
    Tags,
    Assignee,
    DueDate,
    OrderBy,
    Actions,
}

impl FilterPanelSection {
    pub fn next(self) -> Self {
        match self {
            Self::Priority => Self::Tags,
            Self::Tags => Self::Assignee,
            Self::Assignee => Self::DueDate,
            Self::DueDate => Self::OrderBy,
            Self::OrderBy => Self::Actions,
            Self::Actions => Self::Priority,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Self::Priority => Self::Actions,
            Self::Tags => Self::Priority,
            Self::Assignee => Self::Tags,
            Self::DueDate => Self::Assignee,
            Self::OrderBy => Self::DueDate,
            Self::Actions => Self::OrderBy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DueDateMode {
    #[default]
    Before,
    After,
}

impl DueDateMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Before => Self::After,
            Self::After => Self::Before,
        }
    }
}

/// Menu items for Home view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HomeMenuItem {
    #[default]
    Kanban,
    KnowledgeBase,
    WorkspaceSwitch,
    Logout,
}

impl HomeMenuItem {
    pub fn all() -> &'static [Self] {
        &[Self::Kanban, Self::KnowledgeBase, Self::WorkspaceSwitch, Self::Logout]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Kanban => "Kanban Board",
            Self::KnowledgeBase => "Knowledge Base",
            Self::WorkspaceSwitch => "Switch Workspace",
            Self::Logout => "Logout",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Kanban => "📋",
            Self::KnowledgeBase => "📚",
            Self::WorkspaceSwitch => "🔄",
            Self::Logout => "🚪",
        }
    }
}

/// Knowledge Base panel focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KbFocus {
    #[default]
    Tree,    // Left panel - document tree
    Content, // Right panel - document content
}

/// Sort field options for the filter panel
pub const SORT_FIELDS: &[(&str, &str)] = &[
    ("position", "Position"),
    ("title", "Title"),
    ("priority", "Priority"),
    ("due_date", "Due Date"),
    ("created_at", "Created"),
    ("updated_at", "Updated"),
];

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
    pub register_username: String,
    pub register_display_name: String,

    // Email verification
    pub verification_email: String,
    pub verification_code: String,

    // Workspace selection
    pub workspaces: Vec<WorkspaceWithRole>,
    pub selected_workspace_idx: usize,
    pub creating_workspace: bool,
    pub new_workspace_name: String,
    pub accepting_invite: bool,
    pub invite_token_input: String,

    // Home view state
    pub home_menu_idx: usize,
    pub home_quote: Option<String>,
    pub home_quote_author: Option<String>,
    pub home_stats: todo_shared::api::WorkspaceStats,
    pub calendar_year: i32,
    pub calendar_month: u32,
    pub calendar_tasks: std::collections::HashMap<NaiveDate, usize>,

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
    pub task_comments: Vec<CommentWithAuthor>,
    pub adding_comment: bool,
    pub comment_textarea: Option<TextArea<'static>>,

    // Create task state
    pub creating_task: bool,
    pub new_task_title: String,
    pub new_task_description_textarea: Option<TextArea<'static>>,
    pub new_task_field: NewTaskField,

    // Delete task state
    pub confirming_delete: bool,

    // Edit task state
    pub editing_task: bool,
    pub edit_field: TaskEditField,
    pub edit_task_title: String,
    pub edit_task_description_textarea: Option<TextArea<'static>>,
    pub edit_task_priority: Option<Priority>,
    pub edit_task_due_date_str: String,
    pub edit_task_time_estimate_str: String,
    pub edit_task_assignee: Option<uuid::Uuid>,

    // Workspace members (for assignee selection)
    pub workspace_members: Vec<WorkspaceMemberWithUser>,

    // Search state
    pub searching: bool,
    pub search_query: String,
    pub search_results: Vec<SearchResultItem>,
    pub search_total: i64,
    pub search_selected: usize,
    pub search_fuzzy: bool,

    // Filter state
    pub active_filters: TaskListParams,
    pub filter_bar_visible: bool,

    // Filter panel state
    pub filter_panel_visible: bool,
    pub filter_panel_section: FilterPanelSection,
    pub filter_priority_cursor: usize,        // 0=None, 1-5=priorities
    pub filter_tag_cursor: usize,
    pub filter_selected_tags: Vec<uuid::Uuid>,
    pub filter_assignee_cursor: usize,        // 0=None, 1..=N=members
    pub filter_due_mode: DueDateMode,
    pub filter_due_input: String,
    pub filter_order_cursor: usize,           // Index into SORT_FIELDS
    pub filter_order_desc: bool,

    // Preset panel state
    pub preset_panel_visible: bool,
    pub preset_list_cursor: usize,
    pub creating_preset: bool,
    pub new_preset_name: String,

    // Command mode
    pub command_mode: bool,
    pub command_input: String,

    // Filter presets (from preferences)
    pub filter_presets: Vec<FilterPreset>,

    // Tags
    pub workspace_tags: Vec<Tag>,

    // Tag selector in edit mode
    pub task_edit_selected_tags: Vec<uuid::Uuid>,
    pub tag_selector_cursor: usize,

    // Tag management popup
    pub tag_management_visible: bool,
    pub tag_management_cursor: usize,
    pub tag_management_mode: TagManagementMode,
    pub tag_create_name: String,
    pub tag_create_color_idx: usize,
    pub tag_edit_id: Option<uuid::Uuid>,

    // Member panel
    pub member_panel_visible: bool,
    pub selected_member_idx: usize,
    pub inviting_member: bool,
    pub invite_email: String,
    pub invite_role_idx: usize, // 0=Reader, 1=Editor, 2=Admin

    // Knowledge Base state
    pub kb_documents: Vec<Document>,
    pub kb_visible_list: Vec<(Document, usize)>, // (doc, depth) for rendering
    pub kb_selected_idx: usize,
    pub kb_expanded: HashSet<uuid::Uuid>,
    pub kb_selected_doc: Option<Document>,
    pub kb_editing: bool,
    pub kb_content_textarea: Option<TextArea<'static>>,
    pub kb_edit_title: String,
    pub kb_creating: bool,
    pub kb_create_title: String,
    pub kb_create_parent_id: Option<uuid::Uuid>,
    pub kb_confirming_delete: bool,
    pub kb_focus: KbFocus,
    pub kb_scroll_offset: usize,

    // Task-Document linking state
    pub task_linked_documents: Vec<todo_shared::api::LinkedDocument>,
    pub kb_linked_tasks: Vec<todo_shared::api::LinkedTask>,
    pub linking_document_mode: bool,
    pub link_document_cursor: usize,
    pub unlinking_document_mode: bool,
    pub unlink_document_cursor: usize,
    pub linking_task_mode: bool,
    pub link_task_cursor: usize,

    // Menu state
    pub menu_visible: bool,
    pub menu_selected_idx: usize,

    // Workspace modal state
    pub workspace_modal_visible: bool,

    // Help state
    pub help_visible: bool,
    pub help_scroll: usize,

    // Terminal clear flag (set after external editor)
    pub needs_terminal_clear: bool,

    // Connection status
    pub is_connected: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FilterPreset {
    pub name: String,
    pub filters: TaskListParams,
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
            register_username: String::new(),
            register_display_name: String::new(),
            verification_email: String::new(),
            verification_code: String::new(),
            workspaces: Vec::new(),
            selected_workspace_idx: 0,
            creating_workspace: false,
            new_workspace_name: String::new(),
            accepting_invite: false,
            invite_token_input: String::new(),
            home_menu_idx: 0,
            home_quote: None,
            home_quote_author: None,
            home_stats: Default::default(),
            calendar_year: chrono::Local::now().year(),
            calendar_month: chrono::Local::now().month(),
            calendar_tasks: std::collections::HashMap::new(),
            current_workspace: None,
            columns: Vec::new(),
            selected_column: 0,
            selected_task: 0,
            moving_task: false,
            column_scroll_offsets: Vec::new(),
            selected_task_detail: None,
            task_comments: Vec::new(),
            adding_comment: false,
            comment_textarea: None,
            creating_task: false,
            new_task_title: String::new(),
            new_task_description_textarea: None,
            new_task_field: NewTaskField::Title,
            confirming_delete: false,
            editing_task: false,
            edit_field: TaskEditField::Title,
            edit_task_title: String::new(),
            edit_task_description_textarea: None,
            edit_task_priority: None,
            edit_task_due_date_str: String::new(),
            edit_task_time_estimate_str: String::new(),
            edit_task_assignee: None,
            workspace_members: Vec::new(),
            searching: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_total: 0,
            search_selected: 0,
            search_fuzzy: false,
            active_filters: TaskListParams::default(),
            filter_bar_visible: false,
            filter_panel_visible: false,
            filter_panel_section: FilterPanelSection::default(),
            filter_priority_cursor: 0,
            filter_tag_cursor: 0,
            filter_selected_tags: Vec::new(),
            filter_assignee_cursor: 0,
            filter_due_mode: DueDateMode::default(),
            filter_due_input: String::new(),
            filter_order_cursor: 0,
            filter_order_desc: false,
            preset_panel_visible: false,
            preset_list_cursor: 0,
            creating_preset: false,
            new_preset_name: String::new(),
            command_mode: false,
            command_input: String::new(),
            filter_presets: UserPreferences::load()
                .map(|p| p.filter_presets)
                .unwrap_or_default(),
            workspace_tags: Vec::new(),
            task_edit_selected_tags: Vec::new(),
            tag_selector_cursor: 0,
            tag_management_visible: false,
            tag_management_cursor: 0,
            tag_management_mode: TagManagementMode::List,
            tag_create_name: String::new(),
            tag_create_color_idx: 0,
            tag_edit_id: None,
            member_panel_visible: false,
            selected_member_idx: 0,
            inviting_member: false,
            invite_email: String::new(),
            invite_role_idx: 0,
            kb_documents: Vec::new(),
            kb_visible_list: Vec::new(),
            kb_selected_idx: 0,
            kb_expanded: HashSet::new(),
            kb_selected_doc: None,
            kb_editing: false,
            kb_content_textarea: None,
            kb_edit_title: String::new(),
            kb_creating: false,
            kb_create_title: String::new(),
            kb_create_parent_id: None,
            kb_confirming_delete: false,
            kb_focus: KbFocus::Tree,
            kb_scroll_offset: 0,

            task_linked_documents: Vec::new(),
            kb_linked_tasks: Vec::new(),
            linking_document_mode: false,
            link_document_cursor: 0,
            unlinking_document_mode: false,
            unlink_document_cursor: 0,
            linking_task_mode: false,
            link_task_cursor: 0,

            menu_visible: false,
            menu_selected_idx: 0,

            workspace_modal_visible: false,

            help_visible: false,
            help_scroll: 0,

            needs_terminal_clear: false,

            is_connected: true,
        }
    }

    pub fn set_loading(&mut self, loading: bool, message: &str) {
        self.loading = loading;
        self.loading_message = message.to_string();
    }

    /// Check if we're in a text input mode where ? should type '?' instead of opening help
    fn is_text_input_mode(&self) -> bool {
        self.vim_mode == VimMode::Insert
            || self.searching
            || self.command_mode
            || self.creating_task
            || self.kb_creating
            || self.kb_editing
            || self.inviting_member
            || (self.tag_management_visible && self.tag_management_mode != TagManagementMode::List)
            || self.creating_preset
    }

    // ========== TextArea Lifecycle Methods ==========

    /// Initialize textarea for comment input
    fn init_comment_textarea(&mut self) {
        self.comment_textarea = Some(editor::create_textarea("", EditorContext::Comment));
    }

    /// Initialize textarea for new task description
    fn init_new_task_description_textarea(&mut self) {
        self.new_task_description_textarea =
            Some(editor::create_textarea("", EditorContext::NewTaskDescription));
    }

    /// Initialize textarea for editing task description
    fn init_edit_task_description_textarea(&mut self, content: &str) {
        self.edit_task_description_textarea =
            Some(editor::create_textarea(content, EditorContext::TaskDescription));
    }

    /// Initialize textarea for document content
    fn init_kb_content_textarea(&mut self, content: &str) {
        self.kb_content_textarea =
            Some(editor::create_textarea(content, EditorContext::DocumentContent));
    }

    /// Get current comment textarea content as String
    fn get_comment_content(&self) -> String {
        self.comment_textarea
            .as_ref()
            .map(editor::textarea_content)
            .unwrap_or_default()
    }

    /// Get current new task description as String
    fn get_new_task_description(&self) -> String {
        self.new_task_description_textarea
            .as_ref()
            .map(editor::textarea_content)
            .unwrap_or_default()
    }

    /// Get current edit task description as String
    fn get_edit_task_description(&self) -> String {
        self.edit_task_description_textarea
            .as_ref()
            .map(editor::textarea_content)
            .unwrap_or_default()
    }

    /// Get current KB document content as String
    fn get_kb_content(&self) -> String {
        self.kb_content_textarea
            .as_ref()
            .map(editor::textarea_content)
            .unwrap_or_default()
    }

    pub fn set_error(&mut self, message: String) {
        // Detect network errors to update connection status
        if message.contains("Network error") || message.contains("connection") {
            self.is_connected = false;
        }
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

        // Handle help modal if visible (global overlay)
        if self.help_visible {
            self.handle_help_key(key);
            return Ok(false);
        }

        // Global help toggle with ?
        if key.code == KeyCode::Char('?') && !self.is_text_input_mode() {
            self.help_visible = true;
            self.help_scroll = 0;
            return Ok(false);
        }

        match self.view {
            View::Login => self.handle_login_key(key, tx).await,
            View::EmailVerification => self.handle_verification_key(key, tx).await,
            View::VerifyingAuth => Ok(false), // No input during verification
            View::WorkspaceSelect => self.handle_workspace_select_key(key, tx).await,
            View::Home => self.handle_home_key(key, tx).await,
            View::Dashboard => self.handle_dashboard_key(key, tx).await,
            View::TaskDetail => self.handle_task_detail_key(key, tx).await,
            View::KnowledgeBase => self.handle_knowledge_base_key(key, tx).await,
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
                self.login_field = InputField::Username;
            }
            KeyCode::Char('l') if self.vim_mode == VimMode::Normal => {
                self.auth_mode = AuthMode::Login;
                self.login_field = InputField::Email;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.login_field = match (self.auth_mode, self.login_field) {
                    (AuthMode::Login, InputField::Email) => InputField::Password,
                    (AuthMode::Login, InputField::Password) => InputField::Email,
                    (AuthMode::Login, _) => InputField::Email,
                    (AuthMode::Register, InputField::Username) => InputField::Email,
                    (AuthMode::Register, InputField::Email) => InputField::Password,
                    (AuthMode::Register, InputField::Password) => InputField::DisplayName,
                    (AuthMode::Register, InputField::DisplayName) => InputField::Username,
                    (AuthMode::Register, _) => InputField::Username,
                };
            }
            KeyCode::Char('j') | KeyCode::Down if self.vim_mode == VimMode::Normal => {
                self.login_field = match (self.auth_mode, self.login_field) {
                    (AuthMode::Login, InputField::Email) => InputField::Password,
                    (AuthMode::Register, InputField::Username) => InputField::Email,
                    (AuthMode::Register, InputField::Email) => InputField::Password,
                    (AuthMode::Register, InputField::Password) => InputField::DisplayName,
                    _ => self.login_field,
                };
            }
            KeyCode::Char('k') | KeyCode::Up if self.vim_mode == VimMode::Normal => {
                self.login_field = match (self.auth_mode, self.login_field) {
                    (AuthMode::Login, InputField::Password) => InputField::Email,
                    (AuthMode::Register, InputField::Email) => InputField::Username,
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
                        if !self.register_username.is_empty()
                            && !self.login_email.is_empty()
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
                    InputField::Username => self.register_username.push(c),
                    InputField::Email => self.login_email.push(c),
                    InputField::Password => self.login_password.push(c),
                    InputField::DisplayName => self.register_display_name.push(c),
                    InputField::VerificationCode => {} // Not used in login view
                }
            }
            KeyCode::Backspace if self.vim_mode == VimMode::Insert => {
                match self.login_field {
                    InputField::Username => { self.register_username.pop(); }
                    InputField::Email => { self.login_email.pop(); }
                    InputField::Password => { self.login_password.pop(); }
                    InputField::DisplayName => { self.register_display_name.pop(); }
                    InputField::VerificationCode => {} // Not used in login view
                }
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_verification_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        if self.loading {
            return Ok(false);
        }

        match key.code {
            KeyCode::Esc => {
                if self.vim_mode == VimMode::Insert {
                    self.vim_mode = VimMode::Normal;
                } else {
                    // Go back to login
                    self.view = View::Login;
                    self.verification_code.clear();
                }
            }
            KeyCode::Char('i') if self.vim_mode == VimMode::Normal => {
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('r') if self.vim_mode == VimMode::Normal => {
                // Resend verification code
                self.do_resend_verification().await;
            }
            KeyCode::Enter => {
                if !self.verification_code.is_empty() {
                    self.do_verify_email(tx).await;
                }
            }
            KeyCode::Char(c) if self.vim_mode == VimMode::Insert => {
                // Only allow digits for verification code
                if c.is_ascii_digit() && self.verification_code.len() < 6 {
                    self.verification_code.push(c);
                }
            }
            KeyCode::Backspace if self.vim_mode == VimMode::Insert => {
                self.verification_code.pop();
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_home_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        if self.loading {
            return Ok(false);
        }

        // Handle workspace modal
        if self.workspace_modal_visible {
            return self.handle_workspace_modal_key(key, tx).await;
        }

        match key.code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char('j') | KeyCode::Down => {
                let items = HomeMenuItem::all();
                if self.home_menu_idx < items.len() - 1 {
                    self.home_menu_idx += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.home_menu_idx > 0 {
                    self.home_menu_idx -= 1;
                }
            }
            KeyCode::Enter => {
                self.execute_home_menu_action(tx).await;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_workspace_modal().await;
            }
            _ => {}
        }

        Ok(false)
    }

    async fn execute_home_menu_action(&mut self, tx: mpsc::Sender<AppEvent>) {
        let items = HomeMenuItem::all();
        if let Some(&item) = items.get(self.home_menu_idx) {
            match item {
                HomeMenuItem::Kanban => {
                    self.view = View::Dashboard;
                    self.load_workspace_data(tx).await;
                }
                HomeMenuItem::KnowledgeBase => {
                    self.open_knowledge_base().await;
                }
                HomeMenuItem::WorkspaceSwitch => {
                    self.open_workspace_modal().await;
                }
                HomeMenuItem::Logout => {
                    self.do_logout().await;
                }
            }
        }
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

        // Handle invite acceptance mode
        if self.accepting_invite {
            match key.code {
                KeyCode::Esc => {
                    self.accepting_invite = false;
                    self.invite_token_input.clear();
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    if !self.invite_token_input.is_empty() {
                        self.do_accept_invite().await;
                    }
                }
                KeyCode::Char(c) => {
                    self.invite_token_input.push(c);
                }
                KeyCode::Backspace => {
                    self.invite_token_input.pop();
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
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('i') => {
                self.accepting_invite = true;
                self.invite_token_input.clear();
                self.vim_mode = VimMode::Insert;
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
                    let _ = WorkspaceState::save(ws.workspace.id);
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

    async fn do_accept_invite(&mut self) {
        self.set_loading(true, "Accepting invite...");

        let token = self.invite_token_input.clone();

        match self.api.accept_invite(&token).await {
            Ok(workspace) => {
                self.accepting_invite = false;
                self.invite_token_input.clear();
                self.vim_mode = VimMode::Normal;
                self.set_error(format!(
                    "Joined '{}' as {:?}!",
                    workspace.workspace.name, workspace.role
                ));
                self.load_workspaces().await;
            }
            Err(e) => {
                self.set_error(format!("Failed to accept invite: {}", e));
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
        // Handle search popup
        if self.searching {
            return self.handle_search_key(key).await;
        }

        // Handle command mode
        if self.command_mode {
            return self.handle_command_key(key).await;
        }

        // Handle menu popup
        if self.menu_visible {
            return self.handle_menu_key(key, _tx).await;
        }

        // Handle workspace modal
        if self.workspace_modal_visible {
            return self.handle_workspace_modal_key(key, _tx).await;
        }

        // Handle tag management popup
        if self.tag_management_visible {
            return self.handle_tag_management_key(key).await;
        }

        // Handle member panel
        if self.member_panel_visible {
            return self.handle_member_panel_key(key).await;
        }

        // Handle filter panel popup
        if self.filter_panel_visible {
            return self.handle_filter_panel_key(key).await;
        }

        // Handle preset panel popup
        if self.preset_panel_visible {
            return self.handle_preset_panel_key(key).await;
        }

        // Handle create task popup
        if self.creating_task {
            // Description field uses TextArea
            if self.new_task_field == NewTaskField::Description {
                if let Some(ref mut textarea) = self.new_task_description_textarea {
                    match key.code {
                        KeyCode::Esc => {
                            self.creating_task = false;
                            self.new_task_title.clear();
                            self.new_task_description_textarea = None;
                            self.new_task_field = NewTaskField::Title;
                            self.vim_mode = VimMode::Normal;
                        }
                        KeyCode::Tab | KeyCode::BackTab => {
                            self.new_task_field = NewTaskField::Title;
                        }
                        KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                            if !self.new_task_title.is_empty() {
                                self.do_create_task().await;
                            }
                        }
                        KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Ctrl+E: external editor
                            let content = self.get_new_task_description();
                            match editor::launch_external_editor(&content, ".md") {
                                Ok(edited) => {
                                    self.needs_terminal_clear = true;
                                    self.new_task_description_textarea = Some(
                                        editor::create_textarea(&edited, EditorContext::NewTaskDescription),
                                    );
                                }
                                Err(e) => {
                                    self.needs_terminal_clear = true;
                                    self.set_error(format!("Editor failed: {}", e));
                                }
                            }
                        }
                        _ => {
                            textarea.input(key);
                        }
                    }
                    return Ok(false);
                }
            }

            // Title field uses simple string input
            match key.code {
                KeyCode::Esc => {
                    self.creating_task = false;
                    self.new_task_title.clear();
                    self.new_task_description_textarea = None;
                    self.new_task_field = NewTaskField::Title;
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    self.new_task_field = match self.new_task_field {
                        NewTaskField::Title => {
                            // Initialize textarea when switching to description
                            if self.new_task_description_textarea.is_none() {
                                self.init_new_task_description_textarea();
                            }
                            NewTaskField::Description
                        }
                        NewTaskField::Description => NewTaskField::Title,
                    };
                }
                KeyCode::Enter => {
                    if !self.new_task_title.is_empty() {
                        self.do_create_task().await;
                    }
                }
                KeyCode::Char(c) => {
                    if self.new_task_field == NewTaskField::Title {
                        self.new_task_title.push(c);
                    }
                }
                KeyCode::Backspace => {
                    if self.new_task_field == NewTaskField::Title {
                        self.new_task_title.pop();
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
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => {
                // Go back to Home
                self.view = View::Home;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Open workspace modal (Ctrl+W)
                self.open_workspace_modal().await;
            }
            KeyCode::Char('h') | KeyCode::Left => self.move_left(),
            KeyCode::Char('l') | KeyCode::Right => self.move_right(),
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Open Knowledge Base
                self.open_knowledge_base().await;
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Open Menu (command palette)
                self.menu_visible = true;
                self.menu_selected_idx = 0;
            }
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
            KeyCode::Char('/') => {
                // Open search
                self.searching = true;
                self.search_query.clear();
                self.search_results.clear();
                self.search_selected = 0;
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char(':') => {
                // Enter command mode
                self.command_mode = true;
                self.command_input.clear();
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('f') => {
                // Toggle filter bar visibility
                self.filter_bar_visible = !self.filter_bar_visible;
            }
            KeyCode::Char('T') => {
                // Open tag management popup
                self.tag_management_visible = true;
                self.tag_management_cursor = 0;
                self.tag_management_mode = TagManagementMode::List;
                self.tag_create_name.clear();
                self.tag_create_color_idx = 0;
                self.tag_edit_id = None;
            }
            KeyCode::Char('F') => {
                // Open filter panel
                self.open_filter_panel().await;
            }
            KeyCode::Char('P') => {
                // Open preset panel
                self.preset_panel_visible = true;
                self.preset_list_cursor = 0;
                self.creating_preset = false;
                self.new_preset_name.clear();
            }
            KeyCode::Char('M') => {
                // Toggle member panel
                if !self.member_panel_visible {
                    // Load members when opening
                    if let Some(ref workspace) = self.current_workspace {
                        if let Ok(members) = self.api.list_members(workspace.id).await {
                            self.workspace_members = members;
                        }
                    }
                }
                self.member_panel_visible = !self.member_panel_visible;
                self.selected_member_idx = 0;
                self.inviting_member = false;
                self.invite_email.clear();
                self.invite_role_idx = 0;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Go to workspace switcher
                self.go_back_to_workspace_select();
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_search_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.searching = false;
                self.search_query.clear();
                self.search_results.clear();
                self.vim_mode = VimMode::Normal;
            }
            KeyCode::Enter => {
                // Navigate to selected result
                match self.search_results.get(self.search_selected) {
                    Some(SearchResultItem::Task(task_result)) => {
                        self.select_task_by_id(task_result.task.id);
                        self.searching = false;
                        self.search_query.clear();
                        self.search_results.clear();
                        self.vim_mode = VimMode::Normal;
                    }
                    Some(SearchResultItem::Document(doc_result)) => {
                        // Navigate to Knowledge Base and select this document
                        let doc = doc_result.document.clone();
                        self.searching = false;
                        self.search_query.clear();
                        self.search_results.clear();
                        self.vim_mode = VimMode::Normal;
                        // Open KB and select the document
                        self.navigate_to_document(doc).await;
                    }
                    None => {}
                }
            }
            KeyCode::Down | KeyCode::Tab => {
                if !self.search_results.is_empty() {
                    self.search_selected = (self.search_selected + 1) % self.search_results.len();
                }
            }
            KeyCode::Up | KeyCode::BackTab => {
                if !self.search_results.is_empty() {
                    self.search_selected = self
                        .search_selected
                        .checked_sub(1)
                        .unwrap_or(self.search_results.len() - 1);
                }
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Toggle fuzzy search
                self.search_fuzzy = !self.search_fuzzy;
                self.do_search().await;
            }
            KeyCode::Char(c) => {
                self.search_query.push(c);
                self.do_search().await;
            }
            KeyCode::Backspace => {
                self.search_query.pop();
                if self.search_query.is_empty() {
                    self.search_results.clear();
                    self.search_total = 0;
                } else {
                    self.do_search().await;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    async fn do_search(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        if self.search_query.trim().is_empty() {
            self.search_results.clear();
            self.search_total = 0;
            return;
        }

        match self
            .api
            .search(workspace_id, &self.search_query, self.search_fuzzy, Some(1), Some(10))
            .await
        {
            Ok(response) => {
                self.search_total = response.total;
                self.search_results = response.results;
                self.search_selected = 0;
            }
            Err(_) => {
                // Silently ignore search errors
            }
        }
    }

    async fn handle_command_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc => {
                self.command_mode = false;
                self.command_input.clear();
                self.vim_mode = VimMode::Normal;
            }
            KeyCode::Enter => {
                let cmd = self.command_input.clone();
                self.command_mode = false;
                self.command_input.clear();
                self.vim_mode = VimMode::Normal;

                // Parse and execute the command
                if let Err(e) = self.execute_command(&cmd).await {
                    self.set_error(e);
                }
            }
            KeyCode::Char(c) => {
                self.command_input.push(c);
            }
            KeyCode::Backspace => {
                self.command_input.pop();
            }
            _ => {}
        }
        Ok(false)
    }

    async fn handle_tag_management_key(&mut self, key: KeyEvent) -> Result<bool> {
        match self.tag_management_mode {
            TagManagementMode::List => {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        self.tag_management_visible = false;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !self.workspace_tags.is_empty() {
                            self.tag_management_cursor = (self.tag_management_cursor + 1) % self.workspace_tags.len();
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if !self.workspace_tags.is_empty() {
                            self.tag_management_cursor = self.tag_management_cursor
                                .checked_sub(1)
                                .unwrap_or(self.workspace_tags.len().saturating_sub(1));
                        }
                    }
                    KeyCode::Char('n') => {
                        // Create new tag
                        self.tag_management_mode = TagManagementMode::Create;
                        self.tag_create_name.clear();
                        self.tag_create_color_idx = 0;
                        self.vim_mode = VimMode::Insert;
                    }
                    KeyCode::Char('e') => {
                        // Edit selected tag
                        if let Some(tag) = self.workspace_tags.get(self.tag_management_cursor) {
                            self.tag_edit_id = Some(tag.id);
                            self.tag_create_name = tag.name.clone();
                            self.tag_create_color_idx = 0; // Could map color to index
                            self.tag_management_mode = TagManagementMode::Edit;
                            self.vim_mode = VimMode::Insert;
                        }
                    }
                    KeyCode::Char('d') => {
                        // Delete selected tag
                        self.do_delete_tag().await;
                    }
                    _ => {}
                }
            }
            TagManagementMode::Create | TagManagementMode::Edit => {
                match key.code {
                    KeyCode::Esc => {
                        self.tag_management_mode = TagManagementMode::List;
                        self.vim_mode = VimMode::Normal;
                    }
                    KeyCode::Enter => {
                        if self.tag_management_mode == TagManagementMode::Create {
                            self.do_create_tag().await;
                        } else {
                            self.do_edit_tag().await;
                        }
                    }
                    KeyCode::Tab => {
                        // Cycle through colors
                        self.tag_create_color_idx = (self.tag_create_color_idx + 1) % TAG_COLORS.len();
                    }
                    KeyCode::Char(c) => {
                        self.tag_create_name.push(c);
                    }
                    KeyCode::Backspace => {
                        self.tag_create_name.pop();
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    async fn handle_member_panel_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Handle invite input mode
        if self.inviting_member {
            match key.code {
                KeyCode::Esc => {
                    self.inviting_member = false;
                    self.invite_email.clear();
                    self.invite_role_idx = 0;
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    self.do_create_invite().await;
                }
                KeyCode::Tab => {
                    // Cycle through roles: Reader(0) -> Editor(1) -> Admin(2)
                    self.invite_role_idx = (self.invite_role_idx + 1) % 3;
                }
                KeyCode::Char(c) => {
                    self.invite_email.push(c);
                }
                KeyCode::Backspace => {
                    self.invite_email.pop();
                }
                _ => {}
            }
            return Ok(false);
        }

        // Normal member panel mode
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.member_panel_visible = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.workspace_members.is_empty() {
                    self.selected_member_idx =
                        (self.selected_member_idx + 1) % self.workspace_members.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.workspace_members.is_empty() {
                    self.selected_member_idx = self
                        .selected_member_idx
                        .checked_sub(1)
                        .unwrap_or(self.workspace_members.len().saturating_sub(1));
                }
            }
            KeyCode::Char('i') => {
                // Open invite modal
                self.inviting_member = true;
                self.invite_email.clear();
                self.invite_role_idx = 0;
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('r') => {
                // Change role of selected member
                self.do_cycle_member_role().await;
            }
            KeyCode::Char('d') => {
                // Remove selected member
                self.do_remove_member().await;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn do_create_invite(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        if self.invite_email.trim().is_empty() {
            return;
        }

        let role = match self.invite_role_idx {
            0 => todo_shared::WorkspaceRole::Reader,
            1 => todo_shared::WorkspaceRole::Editor,
            _ => todo_shared::WorkspaceRole::Admin,
        };

        match self.api.create_invite(workspace_id, &self.invite_email, role).await {
            Ok(invite) => {
                // Show success message with invite token
                self.set_error(format!(
                    "Invite created! Token: {}",
                    invite.token
                ));
                self.inviting_member = false;
                self.invite_email.clear();
                self.invite_role_idx = 0;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to create invite: {}", e));
            }
        }
    }

    async fn do_cycle_member_role(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let member = match self.workspace_members.get(self.selected_member_idx) {
            Some(m) => m.clone(),
            None => return,
        };

        // Can't change owner's role
        if member.role.is_owner() {
            self.set_error("Cannot change owner's role".to_string());
            return;
        }

        // Cycle role: Reader -> Editor -> Admin -> Reader
        let new_role = match member.role {
            todo_shared::WorkspaceRole::Reader => todo_shared::WorkspaceRole::Editor,
            todo_shared::WorkspaceRole::Editor => todo_shared::WorkspaceRole::Admin,
            todo_shared::WorkspaceRole::Admin => todo_shared::WorkspaceRole::Reader,
            todo_shared::WorkspaceRole::Owner => return, // Can't change owner
        };

        match self.api.update_member_role(workspace_id, member.user_id, new_role).await {
            Ok(updated) => {
                // Update in local list
                if let Some(m) = self.workspace_members.get_mut(self.selected_member_idx) {
                    m.role = updated.role;
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to update role: {}", e));
            }
        }
    }

    async fn do_remove_member(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let member = match self.workspace_members.get(self.selected_member_idx) {
            Some(m) => m.clone(),
            None => return,
        };

        // Can't remove owner
        if member.role.is_owner() {
            self.set_error("Cannot remove workspace owner".to_string());
            return;
        }

        match self.api.remove_member(workspace_id, member.user_id).await {
            Ok(()) => {
                // Remove from local list
                self.workspace_members.remove(self.selected_member_idx);
                if self.selected_member_idx >= self.workspace_members.len() && self.selected_member_idx > 0 {
                    self.selected_member_idx -= 1;
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to remove member: {}", e));
            }
        }
    }

    async fn do_create_tag(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        if self.tag_create_name.trim().is_empty() {
            return;
        }

        let color = TAG_COLORS.get(self.tag_create_color_idx).map(|s| s.to_string());

        match self.api.create_tag(workspace_id, &self.tag_create_name, color.as_deref()).await {
            Ok(tag) => {
                self.workspace_tags.push(tag);
                self.tag_management_mode = TagManagementMode::List;
                self.tag_create_name.clear();
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to create tag: {}", e));
            }
        }
    }

    async fn do_edit_tag(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let tag_id = match self.tag_edit_id {
            Some(id) => id,
            None => return,
        };

        if self.tag_create_name.trim().is_empty() {
            return;
        }

        let color = TAG_COLORS.get(self.tag_create_color_idx).map(|s| s.to_string());

        match self.api.update_tag(workspace_id, tag_id, Some(&self.tag_create_name), color.as_deref()).await {
            Ok(updated_tag) => {
                // Update in workspace_tags
                if let Some(tag) = self.workspace_tags.iter_mut().find(|t| t.id == tag_id) {
                    *tag = updated_tag;
                }
                self.tag_management_mode = TagManagementMode::List;
                self.tag_create_name.clear();
                self.tag_edit_id = None;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to update tag: {}", e));
            }
        }
    }

    async fn do_delete_tag(&mut self) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };

        let tag = match self.workspace_tags.get(self.tag_management_cursor) {
            Some(t) => t.clone(),
            None => return,
        };

        match self.api.delete_tag(workspace_id, tag.id).await {
            Ok(()) => {
                self.workspace_tags.retain(|t| t.id != tag.id);
                // Adjust cursor if needed
                if self.tag_management_cursor >= self.workspace_tags.len() && !self.workspace_tags.is_empty() {
                    self.tag_management_cursor = self.workspace_tags.len() - 1;
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to delete tag: {}", e));
            }
        }
    }

    /// Open the filter panel and populate it with current filter state
    async fn open_filter_panel(&mut self) {
        // Load workspace members for assignee selection
        if let Some(ref workspace) = self.current_workspace {
            if let Ok(members) = self.api.list_members(workspace.id).await {
                self.workspace_members = members;
            }
        }

        self.filter_panel_visible = true;
        self.filter_panel_section = FilterPanelSection::Priority;

        // Initialize from current filters
        self.filter_priority_cursor = match self.active_filters.priority {
            None => 0,
            Some(Priority::Highest) => 1,
            Some(Priority::High) => 2,
            Some(Priority::Medium) => 3,
            Some(Priority::Low) => 4,
            Some(Priority::Lowest) => 5,
        };

        // Initialize tag selection from current filters
        self.filter_selected_tags = self.active_filters.tag_ids.clone().unwrap_or_default();
        self.filter_tag_cursor = 0;

        // Initialize assignee
        self.filter_assignee_cursor = if let Some(assigned_id) = self.active_filters.assigned_to {
            self.workspace_members
                .iter()
                .position(|m| m.user_id == assigned_id)
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };

        // Initialize due date
        if let Some(date) = self.active_filters.due_before {
            self.filter_due_mode = DueDateMode::Before;
            self.filter_due_input = date.to_string();
        } else if let Some(date) = self.active_filters.due_after {
            self.filter_due_mode = DueDateMode::After;
            self.filter_due_input = date.to_string();
        } else {
            self.filter_due_mode = DueDateMode::Before;
            self.filter_due_input.clear();
        }

        // Initialize order by
        self.filter_order_cursor = self.active_filters.order_by
            .as_ref()
            .and_then(|field| SORT_FIELDS.iter().position(|(f, _)| f == field))
            .unwrap_or(0);
        self.filter_order_desc = self.active_filters.order
            .as_ref()
            .map(|o| o == "DESC")
            .unwrap_or(false);
    }

    async fn handle_filter_panel_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Handle insert mode for date input
        if self.vim_mode == VimMode::Insert && self.filter_panel_section == FilterPanelSection::DueDate {
            match key.code {
                KeyCode::Esc => {
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    self.vim_mode = VimMode::Normal;
                }
                // Allow navigation keys to exit insert mode and navigate
                KeyCode::Tab => {
                    self.vim_mode = VimMode::Normal;
                    self.filter_panel_section = self.filter_panel_section.next();
                }
                KeyCode::BackTab => {
                    self.vim_mode = VimMode::Normal;
                    self.filter_panel_section = self.filter_panel_section.prev();
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    self.vim_mode = VimMode::Normal;
                    self.filter_panel_section = self.filter_panel_section.next();
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.vim_mode = VimMode::Normal;
                    self.filter_panel_section = self.filter_panel_section.prev();
                }
                KeyCode::Char(c) => {
                    // Only allow date characters
                    if c.is_ascii_digit() || c == '-' {
                        self.filter_due_input.push(c);
                    }
                }
                KeyCode::Backspace => {
                    self.filter_due_input.pop();
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.filter_panel_visible = false;
            }
            KeyCode::Tab | KeyCode::Char('j') | KeyCode::Down => {
                self.filter_panel_section = self.filter_panel_section.next();
            }
            KeyCode::BackTab | KeyCode::Char('k') | KeyCode::Up => {
                self.filter_panel_section = self.filter_panel_section.prev();
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.filter_panel_prev_value();
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.filter_panel_next_value();
            }
            KeyCode::Char(' ') => {
                // Toggle selection/direction based on section
                match self.filter_panel_section {
                    FilterPanelSection::Tags => {
                        if let Some(tag) = self.workspace_tags.get(self.filter_tag_cursor) {
                            let tag_id = tag.id;
                            if self.filter_selected_tags.contains(&tag_id) {
                                self.filter_selected_tags.retain(|&id| id != tag_id);
                            } else {
                                self.filter_selected_tags.push(tag_id);
                            }
                        }
                    }
                    FilterPanelSection::OrderBy => {
                        self.filter_order_desc = !self.filter_order_desc;
                    }
                    _ => {}
                }
            }
            KeyCode::Char('i') => {
                // Enter insert mode for date
                if self.filter_panel_section == FilterPanelSection::DueDate {
                    self.vim_mode = VimMode::Insert;
                }
            }
            KeyCode::Enter => {
                // Apply filters and close
                self.apply_filter_panel().await;
                self.filter_panel_visible = false;
            }
            KeyCode::Char('c') => {
                // Clear all filters
                self.filter_priority_cursor = 0;
                self.filter_selected_tags.clear();
                self.filter_assignee_cursor = 0;
                self.filter_due_input.clear();
                self.filter_order_cursor = 0;
                self.filter_order_desc = false;
            }
            KeyCode::Char('s') => {
                // Save as preset - open preset panel in create mode
                self.filter_panel_visible = false;
                self.preset_panel_visible = true;
                self.creating_preset = true;
                self.new_preset_name.clear();
                self.vim_mode = VimMode::Insert;
            }
            _ => {}
        }
        Ok(false)
    }

    fn filter_panel_next_value(&mut self) {
        match self.filter_panel_section {
            FilterPanelSection::Priority => {
                self.filter_priority_cursor = (self.filter_priority_cursor + 1) % 6;
            }
            FilterPanelSection::Tags => {
                if !self.workspace_tags.is_empty() {
                    self.filter_tag_cursor = (self.filter_tag_cursor + 1) % self.workspace_tags.len();
                }
            }
            FilterPanelSection::Assignee => {
                let max = self.workspace_members.len() + 1; // +1 for "None"
                self.filter_assignee_cursor = (self.filter_assignee_cursor + 1) % max;
            }
            FilterPanelSection::DueDate => {
                self.filter_due_mode = self.filter_due_mode.toggle();
            }
            FilterPanelSection::OrderBy => {
                self.filter_order_cursor = (self.filter_order_cursor + 1) % SORT_FIELDS.len();
            }
            FilterPanelSection::Actions => {
                // No h/l action in Actions section
            }
        }
    }

    fn filter_panel_prev_value(&mut self) {
        match self.filter_panel_section {
            FilterPanelSection::Priority => {
                self.filter_priority_cursor = self.filter_priority_cursor
                    .checked_sub(1)
                    .unwrap_or(5);
            }
            FilterPanelSection::Tags => {
                if !self.workspace_tags.is_empty() {
                    self.filter_tag_cursor = self.filter_tag_cursor
                        .checked_sub(1)
                        .unwrap_or(self.workspace_tags.len() - 1);
                }
            }
            FilterPanelSection::Assignee => {
                let max = self.workspace_members.len(); // 0 = None, 1..max = members
                self.filter_assignee_cursor = self.filter_assignee_cursor
                    .checked_sub(1)
                    .unwrap_or(max);
            }
            FilterPanelSection::DueDate => {
                self.filter_due_mode = self.filter_due_mode.toggle();
            }
            FilterPanelSection::OrderBy => {
                self.filter_order_cursor = self.filter_order_cursor
                    .checked_sub(1)
                    .unwrap_or(SORT_FIELDS.len() - 1);
            }
            FilterPanelSection::Actions => {
                // No h/l action in Actions section
            }
        }
    }

    async fn apply_filter_panel(&mut self) {
        // Priority
        self.active_filters.priority = match self.filter_priority_cursor {
            0 => None,
            1 => Some(Priority::Highest),
            2 => Some(Priority::High),
            3 => Some(Priority::Medium),
            4 => Some(Priority::Low),
            5 => Some(Priority::Lowest),
            _ => None,
        };

        // Tags
        self.active_filters.tag_ids = if self.filter_selected_tags.is_empty() {
            None
        } else {
            Some(self.filter_selected_tags.clone())
        };

        // Assignee
        self.active_filters.assigned_to = if self.filter_assignee_cursor == 0 {
            None
        } else {
            self.workspace_members
                .get(self.filter_assignee_cursor - 1)
                .map(|m| m.user_id)
        };

        // Due date
        self.active_filters.due_before = None;
        self.active_filters.due_after = None;
        if !self.filter_due_input.is_empty() {
            if let Ok(date) = self.filter_due_input.parse::<NaiveDate>() {
                match self.filter_due_mode {
                    DueDateMode::Before => self.active_filters.due_before = Some(date),
                    DueDateMode::After => self.active_filters.due_after = Some(date),
                }
            }
        }

        // Order by
        if let Some((field, _)) = SORT_FIELDS.get(self.filter_order_cursor) {
            self.active_filters.order_by = Some(field.to_string());
            self.active_filters.order = Some(if self.filter_order_desc { "DESC" } else { "ASC" }.to_string());
        }

        // Show filter bar if any filters active
        self.filter_bar_visible = self.active_filters.priority.is_some()
            || self.active_filters.assigned_to.is_some()
            || self.active_filters.due_before.is_some()
            || self.active_filters.due_after.is_some()
            || self.active_filters.tag_ids.is_some()
            || self.active_filters.order_by.is_some();

        // Reload data with new filters
        self.reload_workspace_data().await;
    }

    async fn handle_preset_panel_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.creating_preset {
            // Handle preset name input
            match key.code {
                KeyCode::Esc => {
                    self.creating_preset = false;
                    self.new_preset_name.clear();
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    if !self.new_preset_name.trim().is_empty() {
                        self.save_current_as_preset();
                    }
                    self.creating_preset = false;
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Char(c) => {
                    self.new_preset_name.push(c);
                }
                KeyCode::Backspace => {
                    self.new_preset_name.pop();
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.preset_panel_visible = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.filter_presets.is_empty() {
                    self.preset_list_cursor = (self.preset_list_cursor + 1) % self.filter_presets.len();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.filter_presets.is_empty() {
                    self.preset_list_cursor = self.preset_list_cursor
                        .checked_sub(1)
                        .unwrap_or(self.filter_presets.len().saturating_sub(1));
                }
            }
            KeyCode::Enter => {
                // Load selected preset
                if let Some(preset) = self.filter_presets.get(self.preset_list_cursor) {
                    self.active_filters = preset.filters.clone();
                    self.filter_bar_visible = true;
                    self.preset_panel_visible = false;
                    self.reload_workspace_data().await;
                }
            }
            KeyCode::Char('n') => {
                // Create new preset from current filters
                self.creating_preset = true;
                self.new_preset_name.clear();
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('d') => {
                // Delete selected preset
                if !self.filter_presets.is_empty() {
                    self.filter_presets.remove(self.preset_list_cursor);
                    if self.preset_list_cursor >= self.filter_presets.len() && !self.filter_presets.is_empty() {
                        self.preset_list_cursor = self.filter_presets.len() - 1;
                    }
                    self.save_presets();
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn save_current_as_preset(&mut self) {
        let preset = FilterPreset {
            name: self.new_preset_name.trim().to_string(),
            filters: self.active_filters.clone(),
        };
        self.filter_presets.push(preset);
        self.new_preset_name.clear();
        self.save_presets();
    }

    fn save_presets(&self) {
        let prefs = UserPreferences {
            filter_presets: self.filter_presets.clone(),
        };
        if let Err(e) = prefs.save() {
            // Log error but don't fail
            eprintln!("Failed to save presets: {}", e);
        }
    }

    async fn execute_command(&mut self, cmd: &str) -> Result<(), String> {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "filter" => self.parse_filter_command(&parts[1..]).await,
            "sort" => self.parse_sort_command(&parts[1..]),
            "clear" => {
                self.active_filters = TaskListParams::default();
                self.filter_bar_visible = false;
                self.reload_workspace_data().await;
                Ok(())
            }
            "preset" => self.parse_preset_command(&parts[1..]).await,
            "q" | "quit" => {
                // This will be handled specially - return error to signal quit
                Err("__QUIT__".to_string())
            }
            _ => Err(format!("Unknown command: {}", parts[0])),
        }
    }

    async fn parse_filter_command(&mut self, args: &[&str]) -> Result<(), String> {
        for arg in args {
            if let Some((key, value)) = arg.split_once('=') {
                match key {
                    "priority" => {
                        self.active_filters.priority = match value.to_lowercase().as_str() {
                            "highest" => Some(Priority::Highest),
                            "high" => Some(Priority::High),
                            "medium" => Some(Priority::Medium),
                            "low" => Some(Priority::Low),
                            "lowest" => Some(Priority::Lowest),
                            "none" => None,
                            _ => return Err(format!("Invalid priority: {}", value)),
                        };
                    }
                    "assigned" | "assignee" => {
                        if value == "me" {
                            self.active_filters.assigned_to = self.user.as_ref().map(|u| u.id);
                        } else if value == "none" {
                            self.active_filters.assigned_to = None;
                        } else {
                            // Try to find member by name
                            let member = self.workspace_members.iter().find(|m| {
                                m.display_name.to_lowercase().contains(&value.to_lowercase())
                            });
                            if let Some(m) = member {
                                self.active_filters.assigned_to = Some(m.user_id);
                            } else {
                                return Err(format!("Member not found: {}", value));
                            }
                        }
                    }
                    "due" | "due_before" => {
                        if let Ok(date) = value.parse::<NaiveDate>() {
                            self.active_filters.due_before = Some(date);
                        } else {
                            return Err(format!("Invalid date format: {}", value));
                        }
                    }
                    "due_after" => {
                        if let Ok(date) = value.parse::<NaiveDate>() {
                            self.active_filters.due_after = Some(date);
                        } else {
                            return Err(format!("Invalid date format: {}", value));
                        }
                    }
                    _ => return Err(format!("Unknown filter: {}", key)),
                }
            } else {
                return Err(format!("Invalid filter syntax: {}", arg));
            }
        }

        self.filter_bar_visible = true;
        self.reload_workspace_data().await;
        Ok(())
    }

    fn parse_sort_command(&mut self, args: &[&str]) -> Result<(), String> {
        if args.is_empty() {
            return Err("Usage: sort <field> or sort -<field> (descending)".to_string());
        }

        let field = args[0];
        let (order_by, descending) = if field.starts_with('-') {
            (&field[1..], true)
        } else {
            (field, false)
        };

        // Validate field name
        match order_by {
            "title" | "priority" | "due_date" | "created_at" | "updated_at" | "position" => {
                self.active_filters.order_by = Some(order_by.to_string());
                self.active_filters.order = Some(if descending { "DESC" } else { "ASC" }.to_string());
                self.filter_bar_visible = true;
                Ok(())
            }
            _ => Err(format!("Invalid sort field: {}. Valid fields: title, priority, due_date, created_at, updated_at, position", order_by)),
        }
    }

    async fn parse_preset_command(&mut self, args: &[&str]) -> Result<(), String> {
        if args.len() < 2 {
            return Err("Usage: preset save <name> or preset load <name>".to_string());
        }

        match args[0] {
            "save" => {
                let name = args[1].to_string();
                let preset = FilterPreset {
                    name: name.clone(),
                    filters: self.active_filters.clone(),
                };
                // Remove existing preset with same name
                self.filter_presets.retain(|p| p.name != name);
                self.filter_presets.push(preset);

                // Save to disk
                let prefs = UserPreferences {
                    filter_presets: self.filter_presets.clone(),
                };
                if let Err(e) = prefs.save() {
                    return Err(format!("Failed to save preferences: {}", e));
                }
                Ok(())
            }
            "load" => {
                let name = args[1];
                if let Some(preset) = self.filter_presets.iter().find(|p| p.name == name) {
                    self.active_filters = preset.filters.clone();
                    self.filter_bar_visible = true;
                    self.reload_workspace_data().await;
                    Ok(())
                } else {
                    Err(format!("Preset not found: {}", name))
                }
            }
            "list" => {
                // Could show preset list - for now just return names
                let names: Vec<_> = self.filter_presets.iter().map(|p| p.name.as_str()).collect();
                if names.is_empty() {
                    Err("No presets saved".to_string())
                } else {
                    Err(format!("Presets: {}", names.join(", ")))
                }
            }
            _ => Err(format!("Unknown preset command: {}", args[0])),
        }
    }

    async fn reload_workspace_data(&mut self) {
        if let Some(ref workspace) = self.current_workspace {
            let workspace_id = workspace.id;

            // Fetch tasks with active filters
            let params = if self.has_active_filters() {
                Some(&self.active_filters)
            } else {
                None
            };

            let statuses = match self.api.list_statuses(workspace_id).await {
                Ok(s) => s,
                Err(_) => return,
            };

            let tasks = match self.api.list_tasks(workspace_id, params).await {
                Ok(response) => response.tasks,
                Err(_) => return,
            };

            self.on_workspace_data_loaded(statuses, tasks);
        }
    }

    fn has_active_filters(&self) -> bool {
        self.active_filters.priority.is_some()
            || self.active_filters.assigned_to.is_some()
            || self.active_filters.due_before.is_some()
            || self.active_filters.due_after.is_some()
            || self.active_filters.q.is_some()
            || self.active_filters.tag_ids.is_some()
            || self.active_filters.order_by.is_some()
    }

    fn select_task_by_id(&mut self, task_id: uuid::Uuid) {
        for (col_idx, column) in self.columns.iter().enumerate() {
            for (task_idx, task) in column.tasks.iter().enumerate() {
                if task.id == task_id {
                    self.selected_column = col_idx;
                    self.selected_task = task_idx;
                    return;
                }
            }
        }
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

        // Handle document linking mode
        if self.linking_document_mode {
            return self.handle_link_document_key(key).await;
        }

        // Handle document unlinking mode
        if self.unlinking_document_mode {
            return self.handle_unlink_document_key(key).await;
        }

        // Handle comment input mode with TextArea
        if self.adding_comment {
            if let Some(ref mut textarea) = self.comment_textarea {
                match key.code {
                    KeyCode::Esc => {
                        self.adding_comment = false;
                        self.comment_textarea = None;
                        self.vim_mode = VimMode::Normal;
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        // Alt+Enter to submit
                        let content = self.get_comment_content();
                        if !content.is_empty() {
                            self.do_add_comment().await;
                        }
                    }
                    KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Ctrl+E: external editor
                        let content = self.get_comment_content();
                        match editor::launch_external_editor(&content, ".md") {
                            Ok(edited) => {
                                self.needs_terminal_clear = true;
                                self.comment_textarea =
                                    Some(editor::create_textarea(&edited, EditorContext::Comment));
                            }
                            Err(e) => {
                                self.needs_terminal_clear = true;
                                self.set_error(format!("Editor failed: {}", e));
                            }
                        }
                    }
                    _ => {
                        // Pass to textarea for normal input handling
                        textarea.input(key);
                    }
                }
                return Ok(false);
            }
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.close_task_detail();
            }
            KeyCode::Char('a') => {
                // Add comment
                self.adding_comment = true;
                self.init_comment_textarea();
                self.vim_mode = VimMode::Insert;
            }
            KeyCode::Char('e') => {
                // Enter edit mode
                self.enter_edit_mode();
            }
            KeyCode::Char('L') => {
                // Link document to task
                self.open_link_document_picker().await;
            }
            KeyCode::Char('U') => {
                // Unlink document from task
                if !self.task_linked_documents.is_empty() {
                    self.unlinking_document_mode = true;
                    self.unlink_document_cursor = 0;
                }
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

    async fn open_link_document_picker(&mut self) {
        // Load documents if not already loaded
        if self.kb_documents.is_empty() {
            if let Some(ref ws) = self.current_workspace {
                match self.api.list_documents(ws.id).await {
                    Ok(docs) => {
                        self.kb_documents = docs;
                    }
                    Err(e) => {
                        self.set_error(format!("Failed to load documents: {}", e));
                        return;
                    }
                }
            }
        }

        // Filter out already linked documents
        let linked_ids: std::collections::HashSet<_> = self.task_linked_documents
            .iter()
            .map(|d| d.document_id)
            .collect();

        let available: Vec<_> = self.kb_documents
            .iter()
            .filter(|d| !linked_ids.contains(&d.id))
            .collect();

        if available.is_empty() {
            self.set_error("No documents available to link".to_string());
            return;
        }

        self.linking_document_mode = true;
        self.link_document_cursor = 0;
    }

    async fn handle_link_document_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Get available documents (not already linked)
        let linked_ids: std::collections::HashSet<_> = self.task_linked_documents
            .iter()
            .map(|d| d.document_id)
            .collect();

        let available: Vec<_> = self.kb_documents
            .iter()
            .filter(|d| !linked_ids.contains(&d.id))
            .collect();

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.linking_document_mode = false;
                self.link_document_cursor = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.link_document_cursor < available.len().saturating_sub(1) {
                    self.link_document_cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.link_document_cursor > 0 {
                    self.link_document_cursor -= 1;
                }
            }
            KeyCode::Enter => {
                if let Some(doc) = available.get(self.link_document_cursor) {
                    self.do_link_document(doc.id).await;
                }
                self.linking_document_mode = false;
                self.link_document_cursor = 0;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn do_link_document(&mut self, doc_id: uuid::Uuid) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };
        let task_id = match self.selected_task_detail {
            Some(ref t) => t.id,
            None => return,
        };

        match self.api.link_task_to_document(workspace_id, doc_id, task_id).await {
            Ok(_) => {
                // Reload linked documents
                match self.api.list_linked_documents(workspace_id, task_id).await {
                    Ok(docs) => {
                        self.task_linked_documents = docs;
                    }
                    Err(_) => {}
                }
            }
            Err(e) => {
                self.set_error(format!("Failed to link document: {}", e));
            }
        }
    }

    async fn handle_unlink_document_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.unlinking_document_mode = false;
                self.unlink_document_cursor = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.unlink_document_cursor < self.task_linked_documents.len().saturating_sub(1) {
                    self.unlink_document_cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.unlink_document_cursor > 0 {
                    self.unlink_document_cursor -= 1;
                }
            }
            KeyCode::Enter => {
                if let Some(doc) = self.task_linked_documents.get(self.unlink_document_cursor) {
                    let doc_id = doc.document_id;
                    self.do_unlink_document(doc_id).await;
                }
                self.unlinking_document_mode = false;
                self.unlink_document_cursor = 0;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn do_unlink_document(&mut self, doc_id: uuid::Uuid) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };
        let task_id = match self.selected_task_detail {
            Some(ref t) => t.id,
            None => return,
        };

        match self.api.unlink_task_from_document(workspace_id, doc_id, task_id).await {
            Ok(_) => {
                // Remove from local list
                self.task_linked_documents.retain(|d| d.document_id != doc_id);
            }
            Err(e) => {
                self.set_error(format!("Failed to unlink document: {}", e));
            }
        }
    }

    async fn handle_edit_task_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Insert mode - editing current field
        if self.vim_mode == VimMode::Insert {
            // Special handling for description field with TextArea
            if self.edit_field == TaskEditField::Description {
                if let Some(ref mut textarea) = self.edit_task_description_textarea {
                    match key.code {
                        KeyCode::Esc => {
                            self.vim_mode = VimMode::Normal;
                        }
                        KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            // Ctrl+E: external editor
                            let content = self.get_edit_task_description();
                            match editor::launch_external_editor(&content, ".md") {
                                Ok(edited) => {
                                    self.needs_terminal_clear = true;
                                    self.edit_task_description_textarea = Some(
                                        editor::create_textarea(&edited, EditorContext::TaskDescription),
                                    );
                                }
                                Err(e) => {
                                    self.needs_terminal_clear = true;
                                    self.set_error(format!("Editor failed: {}", e));
                                }
                            }
                        }
                        _ => {
                            // Pass to textarea for normal input handling
                            textarea.input(key);
                        }
                    }
                    return Ok(false);
                }
            }

            // Other fields use simple string handling
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
                        TaskEditField::Description => {} // Handled above with TextArea
                        TaskEditField::DueDate => self.edit_task_due_date_str.push(c),
                        TaskEditField::TimeEstimate => self.edit_task_time_estimate_str.push(c),
                        TaskEditField::Priority | TaskEditField::Assignee | TaskEditField::Tags => {}
                    }
                }
                KeyCode::Backspace => {
                    match self.edit_field {
                        TaskEditField::Title => { self.edit_task_title.pop(); }
                        TaskEditField::Description => {} // Handled above with TextArea
                        TaskEditField::DueDate => { self.edit_task_due_date_str.pop(); }
                        TaskEditField::TimeEstimate => { self.edit_task_time_estimate_str.pop(); }
                        TaskEditField::Priority | TaskEditField::Assignee | TaskEditField::Tags => {}
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
                // Enter insert mode for current field (except Priority, Assignee, Tags)
                if self.edit_field != TaskEditField::Priority
                    && self.edit_field != TaskEditField::Assignee
                    && self.edit_field != TaskEditField::Tags {
                    self.vim_mode = VimMode::Insert;
                }
            }
            KeyCode::Tab => {
                // Next field
                self.edit_field = match self.edit_field {
                    TaskEditField::Title => TaskEditField::Description,
                    TaskEditField::Description => TaskEditField::Priority,
                    TaskEditField::Priority => TaskEditField::DueDate,
                    TaskEditField::DueDate => TaskEditField::TimeEstimate,
                    TaskEditField::TimeEstimate => TaskEditField::Assignee,
                    TaskEditField::Assignee => TaskEditField::Tags,
                    TaskEditField::Tags => TaskEditField::Title,
                };
            }
            KeyCode::BackTab => {
                // Previous field
                self.edit_field = match self.edit_field {
                    TaskEditField::Title => TaskEditField::Tags,
                    TaskEditField::Description => TaskEditField::Title,
                    TaskEditField::Priority => TaskEditField::Description,
                    TaskEditField::DueDate => TaskEditField::Priority,
                    TaskEditField::TimeEstimate => TaskEditField::DueDate,
                    TaskEditField::Assignee => TaskEditField::TimeEstimate,
                    TaskEditField::Tags => TaskEditField::Assignee,
                };
            }
            KeyCode::Char('l') | KeyCode::Right if self.edit_field == TaskEditField::Tags => {
                // Navigate to next tag
                if !self.workspace_tags.is_empty() {
                    self.tag_selector_cursor = (self.tag_selector_cursor + 1) % self.workspace_tags.len();
                }
            }
            KeyCode::Char('h') | KeyCode::Left if self.edit_field == TaskEditField::Tags => {
                // Navigate to previous tag
                if !self.workspace_tags.is_empty() {
                    self.tag_selector_cursor = self.tag_selector_cursor
                        .checked_sub(1)
                        .unwrap_or(self.workspace_tags.len().saturating_sub(1));
                }
            }
            KeyCode::Char(' ') if self.edit_field == TaskEditField::Tags => {
                // Toggle tag selection
                if let Some(tag) = self.workspace_tags.get(self.tag_selector_cursor) {
                    let tag_id = tag.id;
                    if self.task_edit_selected_tags.contains(&tag_id) {
                        self.task_edit_selected_tags.retain(|&id| id != tag_id);
                    } else {
                        self.task_edit_selected_tags.push(tag_id);
                    }
                }
            }
            KeyCode::Char('j') | KeyCode::Down => {
                // Next field (for non-Tags fields)
                self.edit_field = match self.edit_field {
                    TaskEditField::Title => TaskEditField::Description,
                    TaskEditField::Description => TaskEditField::Priority,
                    TaskEditField::Priority => TaskEditField::DueDate,
                    TaskEditField::DueDate => TaskEditField::TimeEstimate,
                    TaskEditField::TimeEstimate => TaskEditField::Assignee,
                    TaskEditField::Assignee => TaskEditField::Tags,
                    TaskEditField::Tags => TaskEditField::Title,
                };
            }
            KeyCode::Char('k') | KeyCode::Up => {
                // Previous field (for non-Tags fields)
                self.edit_field = match self.edit_field {
                    TaskEditField::Title => TaskEditField::Tags,
                    TaskEditField::Description => TaskEditField::Title,
                    TaskEditField::Priority => TaskEditField::Description,
                    TaskEditField::DueDate => TaskEditField::Priority,
                    TaskEditField::TimeEstimate => TaskEditField::DueDate,
                    TaskEditField::Assignee => TaskEditField::TimeEstimate,
                    TaskEditField::Tags => TaskEditField::Assignee,
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
            KeyCode::Char('h') | KeyCode::Left if self.edit_field == TaskEditField::Assignee => {
                // Previous assignee (or none)
                if self.workspace_members.is_empty() {
                    self.edit_task_assignee = None;
                } else {
                    let current_idx = self.edit_task_assignee
                        .and_then(|id| self.workspace_members.iter().position(|m| m.user_id == id));
                    self.edit_task_assignee = match current_idx {
                        None => Some(self.workspace_members.last().unwrap().user_id),
                        Some(0) => None, // Go to "none"
                        Some(i) => Some(self.workspace_members[i - 1].user_id),
                    };
                }
            }
            KeyCode::Char('l') | KeyCode::Right if self.edit_field == TaskEditField::Assignee => {
                // Next assignee
                if self.workspace_members.is_empty() {
                    self.edit_task_assignee = None;
                } else {
                    let current_idx = self.edit_task_assignee
                        .and_then(|id| self.workspace_members.iter().position(|m| m.user_id == id));
                    self.edit_task_assignee = match current_idx {
                        None => Some(self.workspace_members[0].user_id),
                        Some(i) if i + 1 >= self.workspace_members.len() => None, // Wrap to "none"
                        Some(i) => Some(self.workspace_members[i + 1].user_id),
                    };
                }
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

    /// Open workspace modal and refresh the workspace list
    async fn open_workspace_modal(&mut self) {
        // Refresh workspace list before showing modal
        self.load_workspaces().await;

        // Find current workspace in the list
        if let Some(ref current) = self.current_workspace {
            if let Some(idx) = self.workspaces.iter().position(|w| w.workspace.id == current.id) {
                self.selected_workspace_idx = idx;
            }
        }

        self.workspace_modal_visible = true;
        self.creating_workspace = false;
        self.new_workspace_name.clear();
    }

    /// Handle key events in the workspace modal
    async fn handle_workspace_modal_key(
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
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    if !self.new_workspace_name.is_empty() {
                        self.do_create_workspace_modal().await;
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
            KeyCode::Esc | KeyCode::Char('q') => {
                self.workspace_modal_visible = false;
            }
            KeyCode::Char('n') => {
                self.creating_workspace = true;
                self.new_workspace_name.clear();
                self.vim_mode = VimMode::Insert;
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
                    let _ = WorkspaceState::save(ws.workspace.id);
                    self.workspace_modal_visible = false;
                    self.load_workspace_data(tx).await;
                }
            }
            KeyCode::Char('L') => {
                self.workspace_modal_visible = false;
                self.do_logout().await;
            }
            _ => {}
        }

        Ok(false)
    }

    /// Create workspace from modal
    async fn do_create_workspace_modal(&mut self) {
        self.set_loading(true, "Creating workspace...");

        let name = self.new_workspace_name.clone();

        match self.api.create_workspace(&name, None).await {
            Ok(ws) => {
                self.creating_workspace = false;
                self.new_workspace_name.clear();
                self.vim_mode = VimMode::Normal;
                // Add new workspace to list and select it
                self.workspaces.push(WorkspaceWithRole {
                    workspace: ws.clone(),
                    role: todo_shared::WorkspaceRole::Owner,
                });
                self.selected_workspace_idx = self.workspaces.len() - 1;
            }
            Err(e) => {
                self.set_error(format!("Failed to create workspace: {}", e));
            }
        }

        self.set_loading(false, "");
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

    async fn do_register(&mut self, _tx: mpsc::Sender<AppEvent>) {
        self.set_loading(true, "Registering...");

        let username = self.register_username.clone();
        let email = self.login_email.clone();
        let password = self.login_password.clone();
        let display_name = self.register_display_name.clone();

        match self.api.register(&username, &email, &password, &display_name).await {
            Ok(response) => {
                // Store email for verification
                self.verification_email = response.email;
                self.verification_code.clear();
                self.login_field = InputField::VerificationCode;
                self.vim_mode = VimMode::Normal;
                self.view = View::EmailVerification;
            }
            Err(e) => {
                self.set_error(format!("Registration failed: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    async fn do_verify_email(&mut self, tx: mpsc::Sender<AppEvent>) {
        self.set_loading(true, "Verifying email...");

        let email = self.verification_email.clone();
        let code = self.verification_code.clone();

        match self.api.verify_email(&email, &code).await {
            Ok(user) => {
                self.user = Some(user);
                let _ = tx.send(AppEvent::AuthSuccess).await;
            }
            Err(e) => {
                self.set_error(format!("Verification failed: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    async fn do_resend_verification(&mut self) {
        self.set_loading(true, "Resending verification code...");

        let email = self.verification_email.clone();

        match self.api.resend_verification(&email).await {
            Ok(()) => {
                self.set_error("Verification code resent. Check server logs.".to_string());
            }
            Err(e) => {
                self.set_error(format!("Failed to resend: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    pub async fn verify_auth(&mut self) {
        self.set_loading(true, "Verifying authentication...");

        match self.api.me().await {
            Ok(user) => {
                self.user = Some(user);
                self.load_workspaces().await;

                // Try to restore last opened workspace
                let last_workspace_id = WorkspaceState::load()
                    .ok()
                    .flatten()
                    .map(|s| s.workspace_id);

                let restored = if let Some(id) = last_workspace_id {
                    self.workspaces
                        .iter()
                        .enumerate()
                        .find(|(_, w)| w.workspace.id == id)
                        .map(|(idx, ws)| {
                            self.selected_workspace_idx = idx;
                            self.current_workspace = Some(ws.workspace.clone());
                        })
                        .is_some()
                } else {
                    false
                };

                // Fallback to first workspace if not restored
                if !restored {
                    if let Some(ws) = self.workspaces.first() {
                        self.current_workspace = Some(ws.workspace.clone());
                        self.selected_workspace_idx = 0;
                    }
                }

                if self.current_workspace.is_some() {
                    self.view = View::Home;
                    self.load_home_data().await;
                } else {
                    // No workspaces available
                    self.view = View::WorkspaceSelect;
                }
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
        self.login_password.clear();
        self.load_workspaces().await;

        // Try to restore last opened workspace
        let last_workspace_id = WorkspaceState::load()
            .ok()
            .flatten()
            .map(|s| s.workspace_id);

        let restored = if let Some(id) = last_workspace_id {
            self.workspaces
                .iter()
                .enumerate()
                .find(|(_, w)| w.workspace.id == id)
                .map(|(idx, ws)| {
                    self.selected_workspace_idx = idx;
                    self.current_workspace = Some(ws.workspace.clone());
                })
                .is_some()
        } else {
            false
        };

        // Fallback to first workspace if not restored
        if !restored {
            if let Some(ws) = self.workspaces.first() {
                self.current_workspace = Some(ws.workspace.clone());
                self.selected_workspace_idx = 0;
            }
        }

        if self.current_workspace.is_some() {
            self.view = View::Home;
            self.load_home_data().await;
        } else {
            // No workspaces available
            self.view = View::WorkspaceSelect;
        }
    }

    async fn load_home_data(&mut self) {
        // Load quote asynchronously (don't block UI)
        let (quote, author) = crate::api::quote::get_quote().await;
        self.home_quote = Some(quote);
        self.home_quote_author = Some(author);

        // Load workspace stats and calendar tasks
        if let Some(ref workspace) = self.current_workspace {
            if let Ok(stats) = self.api.get_workspace_stats(workspace.id).await {
                self.home_stats = stats;
            }

            // Load tasks for calendar (current month)
            self.load_calendar_tasks().await;
        }
    }

    async fn load_calendar_tasks(&mut self) {
        let Some(ref workspace) = self.current_workspace else { return };

        // Calculate date range for current calendar month
        let first_day = match NaiveDate::from_ymd_opt(self.calendar_year, self.calendar_month, 1) {
            Some(d) => d,
            None => return,
        };

        // Get last day of month
        let last_day = if self.calendar_month == 12 {
            NaiveDate::from_ymd_opt(self.calendar_year + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(self.calendar_year, self.calendar_month + 1, 1)
        }
        .and_then(|d| d.pred_opt())
        .unwrap_or(first_day);

        // Fetch tasks with due dates in this range
        let params = TaskListParams {
            due_after: Some(first_day),
            due_before: Some(last_day),
            limit: Some(500),
            ..Default::default()
        };

        if let Ok(response) = self.api.list_tasks(workspace.id, Some(&params)).await {
            self.calendar_tasks.clear();
            for task in response.tasks {
                if let Some(due_date) = task.due_date {
                    *self.calendar_tasks.entry(due_date).or_insert(0) += 1;
                }
            }
        }
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
        self.is_connected = true;
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

        // Load workspace tags
        self.workspace_tags = match self.api.list_tags(workspace_id).await {
            Ok(tags) => tags,
            Err(_) => Vec::new(), // Silently fail for tags
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

        // Initialize scroll offsets for each column
        self.column_scroll_offsets = vec![0; self.columns.len()];

        self.selected_column = 0;
        self.selected_task = 0;
        self.view = View::Dashboard;
        self.is_connected = true;
        self.set_loading(false, "");
    }

    pub fn move_left(&mut self) {
        if self.selected_column > 0 {
            self.selected_column -= 1;
            self.selected_task = 0;
            // Reset scroll for new column
            if let Some(offset) = self.column_scroll_offsets.get_mut(self.selected_column) {
                *offset = 0;
            }
        }
    }

    pub fn move_right(&mut self) {
        if !self.columns.is_empty() && self.selected_column < self.columns.len() - 1 {
            self.selected_column += 1;
            self.selected_task = 0;
            // Reset scroll for new column
            if let Some(offset) = self.column_scroll_offsets.get_mut(self.selected_column) {
                *offset = 0;
            }
        }
    }

    pub fn move_up(&mut self) {
        if self.selected_task > 0 {
            self.selected_task -= 1;
            // Adjust scroll if selection is above visible area
            if let Some(offset) = self.column_scroll_offsets.get_mut(self.selected_column) {
                if self.selected_task < *offset {
                    *offset = self.selected_task;
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        if let Some(column) = self.columns.get(self.selected_column) {
            if self.selected_task < column.tasks.len().saturating_sub(1) {
                self.selected_task += 1;
                // Adjust scroll if selection is below visible area
                // Assume ~3 tasks visible per column (conservative estimate)
                // The actual visible count depends on terminal height
                if let Some(offset) = self.column_scroll_offsets.get_mut(self.selected_column) {
                    let visible_tasks = 5; // Conservative default, UI will handle actual rendering
                    if self.selected_task >= *offset + visible_tasks {
                        *offset = self.selected_task.saturating_sub(visible_tasks - 1);
                    }
                }
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

        // Load workspace members for assignee selection
        match self.api.list_members(workspace_id).await {
            Ok(members) => {
                self.workspace_members = members;
            }
            Err(_) => {
                // Non-critical, continue without members
                self.workspace_members.clear();
            }
        }

        // Load linked documents
        match self.api.list_linked_documents(workspace_id, task.id).await {
            Ok(docs) => {
                self.task_linked_documents = docs;
            }
            Err(_) => {
                // Non-critical, continue without linked documents
                self.task_linked_documents.clear();
            }
        }

        self.selected_task_detail = Some(task);
        self.view = View::TaskDetail;
        self.set_loading(false, "");
    }

    fn close_task_detail(&mut self) {
        self.selected_task_detail = None;
        self.task_comments.clear();
        self.task_linked_documents.clear();
        self.adding_comment = false;
        self.comment_textarea = None;
        self.linking_document_mode = false;
        self.link_document_cursor = 0;
        self.unlinking_document_mode = false;
        self.unlink_document_cursor = 0;
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

        let content = self.get_comment_content();

        match self.api.create_comment(workspace_id, task_id, &content).await {
            Ok(comment) => {
                self.task_comments.push(comment);
                self.comment_textarea = None;
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

        // Determine default assignee: workspace setting > self
        let assigned_to = self.current_workspace
            .as_ref()
            .and_then(|ws| ws.settings.default_assignee)
            .or_else(|| self.user.as_ref().map(|u| u.id));

        let description = self.get_new_task_description();
        let req = CreateTaskRequest {
            title: self.new_task_title.clone(),
            status_id,
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            priority: None,
            due_date: None,
            time_estimate_minutes: None,
            assigned_to,
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
                self.new_task_description_textarea = None;
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
        // Extract all values first to avoid borrow conflicts
        let task_data = self.selected_task_detail.as_ref().map(|task| {
            (
                task.title.clone(),
                task.description.clone().unwrap_or_default(),
                task.priority,
                task.due_date.map(|d| d.to_string()).unwrap_or_default(),
                task.time_estimate_minutes.map(|m| m.to_string()).unwrap_or_default(),
                task.assigned_to,
                task.tags.iter().map(|t| t.id).collect::<Vec<_>>(),
            )
        });

        if let Some((title, description, priority, due_date, time_estimate, assignee, tags)) = task_data {
            self.editing_task = true;
            self.edit_field = TaskEditField::Title;
            self.edit_task_title = title;
            self.init_edit_task_description_textarea(&description);
            self.edit_task_priority = priority;
            self.edit_task_due_date_str = due_date;
            self.edit_task_time_estimate_str = time_estimate;
            self.edit_task_assignee = assignee;
            self.task_edit_selected_tags = tags;
            self.tag_selector_cursor = 0;
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

        self.set_loading(true, "Updating task...");

        // Try to update, refresh token if unauthorized
        let result = self.update_task_with_retry(workspace_id, task_id, due_date, time_estimate_minutes).await;

        match result {
            Ok(mut updated_task) => {
                // Also update tags
                let tag_ids = self.task_edit_selected_tags.clone();
                if let Err(e) = self.api.set_task_tags(workspace_id, task_id, tag_ids.clone()).await {
                    self.set_error(format!("Failed to update tags: {}", e));
                } else {
                    // Update tags in the task
                    updated_task.tags = self.workspace_tags
                        .iter()
                        .filter(|t| tag_ids.contains(&t.id))
                        .cloned()
                        .collect();
                }

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

    async fn update_task_with_retry(
        &mut self,
        workspace_id: uuid::Uuid,
        task_id: uuid::Uuid,
        due_date: Option<NaiveDate>,
        time_estimate_minutes: Option<i32>,
    ) -> Result<Task, crate::api::ApiError> {
        let description = self.get_edit_task_description();
        let req = UpdateTaskRequest {
            title: Some(self.edit_task_title.clone()),
            status_id: None,
            description: if description.is_empty() {
                None
            } else {
                Some(description)
            },
            priority: self.edit_task_priority,
            due_date,
            time_estimate_minutes,
            assigned_to: self.edit_task_assignee,
        };

        // First attempt
        match self.api.update_task(workspace_id, task_id, req.clone()).await {
            Ok(task) => Ok(task),
            Err(crate::api::ApiError::Unauthorized) => {
                // Try to refresh token
                if self.api.refresh().await.is_ok() {
                    // Retry with new token
                    self.api.update_task(workspace_id, task_id, req).await
                } else {
                    Err(crate::api::ApiError::Unauthorized)
                }
            }
            Err(e) => Err(e),
        }
    }

    // ============ Knowledge Base ============

    async fn open_knowledge_base(&mut self) {
        let workspace_id = match &self.current_workspace {
            Some(w) => w.id,
            None => return,
        };

        self.set_loading(true, "Loading knowledge base...");

        match self.api.list_documents(workspace_id).await {
            Ok(docs) => {
                self.kb_documents = docs;
                self.build_kb_visible_list();
                self.kb_selected_idx = 0;
                self.kb_selected_doc = self.kb_visible_list.first().map(|(d, _)| d.clone());
                self.load_kb_linked_tasks().await;
                self.view = View::KnowledgeBase;
            }
            Err(e) => {
                self.set_error(format!("Failed to load documents: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    fn build_kb_visible_list(&mut self) {
        self.kb_visible_list.clear();

        // Clone documents to avoid borrow issues
        let docs = self.kb_documents.clone();
        let expanded = self.kb_expanded.clone();

        // Build the list iteratively using a stack
        let mut stack: Vec<(Document, usize)> = Vec::new();

        // Get root documents (no parent)
        let mut roots: Vec<Document> = docs
            .iter()
            .filter(|d| d.parent_id.is_none())
            .cloned()
            .collect();
        roots.sort_by(|a, b| b.title.cmp(&a.title)); // Reverse for stack

        for doc in roots {
            stack.push((doc, 0));
        }

        while let Some((doc, depth)) = stack.pop() {
            let doc_id = doc.id;
            self.kb_visible_list.push((doc, depth));

            // If expanded, add children
            if expanded.contains(&doc_id) {
                let mut children: Vec<Document> = docs
                    .iter()
                    .filter(|d| d.parent_id == Some(doc_id))
                    .cloned()
                    .collect();
                children.sort_by(|a, b| b.title.cmp(&a.title)); // Reverse for stack

                for child in children {
                    stack.push((child, depth + 1));
                }
            }
        }
    }


    async fn handle_knowledge_base_key(
        &mut self,
        key: KeyEvent,
        tx: mpsc::Sender<AppEvent>,
    ) -> Result<bool> {
        // Handle workspace modal
        if self.workspace_modal_visible {
            return self.handle_workspace_modal_key(key, tx).await;
        }

        // Handle linking task mode
        if self.linking_task_mode {
            return self.handle_link_task_key(key).await;
        }

        // Handle delete confirmation
        if self.kb_confirming_delete {
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.do_delete_document().await;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.kb_confirming_delete = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        // Handle create document
        if self.kb_creating {
            match key.code {
                KeyCode::Esc => {
                    self.kb_creating = false;
                    self.kb_create_title.clear();
                    self.kb_create_parent_id = None;
                    self.vim_mode = VimMode::Normal;
                }
                KeyCode::Enter => {
                    if !self.kb_create_title.is_empty() {
                        self.do_create_document().await;
                    }
                }
                KeyCode::Char(c) => {
                    self.kb_create_title.push(c);
                }
                KeyCode::Backspace => {
                    self.kb_create_title.pop();
                }
                _ => {}
            }
            return Ok(false);
        }

        // Handle editing document with TextArea
        if self.kb_editing {
            if let Some(ref mut textarea) = self.kb_content_textarea {
                match key.code {
                    KeyCode::Esc => {
                        self.kb_editing = false;
                        self.kb_edit_title.clear();
                        self.kb_content_textarea = None;
                        self.vim_mode = VimMode::Normal;
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                        // Save on Alt+Enter
                        self.do_update_document().await;
                    }
                    KeyCode::Char('e') | KeyCode::Char('E') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Ctrl+E: external editor
                        let content = self.get_kb_content();
                        match editor::launch_external_editor(&content, ".md") {
                            Ok(edited) => {
                                self.needs_terminal_clear = true;
                                self.kb_content_textarea = Some(
                                    editor::create_textarea(&edited, EditorContext::DocumentContent),
                                );
                            }
                            Err(e) => {
                                self.needs_terminal_clear = true;
                                self.set_error(format!("Editor failed: {}", e));
                            }
                        }
                    }
                    _ => {
                        // Pass to textarea for normal input handling
                        textarea.input(key);
                    }
                }
                return Ok(false);
            }
        }

        // Global keys (work in both panels)
        match key.code {
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.open_workspace_modal().await;
                return Ok(false);
            }
            KeyCode::Char('q') | KeyCode::Esc | KeyCode::Backspace => {
                self.view = View::Home;
                self.kb_selected_doc = None;
                self.kb_focus = KbFocus::Tree;
                self.kb_scroll_offset = 0;
                return Ok(false);
            }
            KeyCode::Tab => {
                // Switch focus between Tree and Content
                self.kb_focus = match self.kb_focus {
                    KbFocus::Tree => KbFocus::Content,
                    KbFocus::Content => KbFocus::Tree,
                };
                return Ok(false);
            }
            KeyCode::Char('n') => {
                // Create new document (global)
                self.kb_creating = true;
                self.kb_create_title.clear();
                if let Some((doc, _)) = self.kb_visible_list.get(self.kb_selected_idx) {
                    if self.kb_expanded.contains(&doc.id) {
                        self.kb_create_parent_id = Some(doc.id);
                    } else {
                        self.kb_create_parent_id = doc.parent_id;
                    }
                }
                self.vim_mode = VimMode::Insert;
                return Ok(false);
            }
            _ => {}
        }

        // Focus-specific keys
        match self.kb_focus {
            KbFocus::Tree => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if !self.kb_visible_list.is_empty() {
                            self.kb_selected_idx = (self.kb_selected_idx + 1).min(self.kb_visible_list.len() - 1);
                            self.kb_selected_doc = self.kb_visible_list.get(self.kb_selected_idx).map(|(d, _)| d.clone());
                            self.kb_scroll_offset = 0; // Reset scroll when selecting new doc
                            self.load_kb_linked_tasks().await;
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if self.kb_selected_idx > 0 {
                            self.kb_selected_idx -= 1;
                            self.kb_selected_doc = self.kb_visible_list.get(self.kb_selected_idx).map(|(d, _)| d.clone());
                            self.kb_scroll_offset = 0; // Reset scroll when selecting new doc
                            self.load_kb_linked_tasks().await;
                        }
                    }
                    KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                        // Expand node
                        if let Some((doc, _)) = self.kb_visible_list.get(self.kb_selected_idx) {
                            if !self.kb_expanded.contains(&doc.id) {
                                self.kb_expanded.insert(doc.id);
                                self.build_kb_visible_list();
                            }
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        // Collapse node or go to parent
                        if let Some((doc, _)) = self.kb_visible_list.get(self.kb_selected_idx).cloned() {
                            if self.kb_expanded.contains(&doc.id) {
                                self.kb_expanded.remove(&doc.id);
                                self.build_kb_visible_list();
                            } else if let Some(parent_id) = doc.parent_id {
                                if let Some(pos) = self.kb_visible_list.iter().position(|(d, _)| d.id == parent_id) {
                                    self.kb_selected_idx = pos;
                                    self.kb_selected_doc = self.kb_visible_list.get(pos).map(|(d, _)| d.clone());
                                }
                            }
                        }
                    }
                    KeyCode::Char('e') => {
                        if let Some(doc) = &self.kb_selected_doc {
                            self.kb_editing = true;
                            self.kb_edit_title = doc.title.clone();
                            let content = doc.content.clone().unwrap_or_default();
                            self.init_kb_content_textarea(&content);
                            self.vim_mode = VimMode::Insert;
                        }
                    }
                    KeyCode::Char('d') => {
                        if self.kb_selected_doc.is_some() {
                            self.kb_confirming_delete = true;
                        }
                    }
                    KeyCode::Char('L') => {
                        if self.kb_selected_doc.is_some() {
                            self.open_link_task_picker().await;
                        }
                    }
                    KeyCode::Char('U') => {
                        if !self.kb_linked_tasks.is_empty() {
                            self.unlink_task_from_kb().await;
                        }
                    }
                    _ => {}
                }
            }
            KbFocus::Content => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        self.kb_scroll_offset = self.kb_scroll_offset.saturating_add(1);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.kb_scroll_offset = self.kb_scroll_offset.saturating_sub(1);
                    }
                    KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Scroll down half page (10 lines)
                        self.kb_scroll_offset = self.kb_scroll_offset.saturating_add(10);
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        // Scroll up half page (10 lines)
                        self.kb_scroll_offset = self.kb_scroll_offset.saturating_sub(10);
                    }
                    KeyCode::Char('g') => {
                        // Scroll to top
                        self.kb_scroll_offset = 0;
                    }
                    KeyCode::Char('G') => {
                        // Scroll to bottom (will be clamped in UI)
                        self.kb_scroll_offset = usize::MAX;
                    }
                    _ => {}
                }
            }
        }

        Ok(false)
    }

    fn get_all_tasks(&self) -> Vec<&Task> {
        self.columns.iter().flat_map(|c| c.tasks.iter()).collect()
    }

    async fn open_link_task_picker(&mut self) {
        // Filter out already linked tasks
        let linked_ids: std::collections::HashSet<_> = self.kb_linked_tasks
            .iter()
            .map(|t| t.task_id)
            .collect();

        let available: Vec<_> = self.get_all_tasks()
            .into_iter()
            .filter(|t| !linked_ids.contains(&t.id))
            .collect::<Vec<_>>();

        if available.is_empty() {
            self.set_error("No tasks available to link".to_string());
            return;
        }

        self.linking_task_mode = true;
        self.link_task_cursor = 0;
    }

    async fn handle_link_task_key(&mut self, key: KeyEvent) -> Result<bool> {
        // Get available tasks (not already linked)
        let linked_ids: std::collections::HashSet<_> = self.kb_linked_tasks
            .iter()
            .map(|t| t.task_id)
            .collect();

        let all_tasks = self.get_all_tasks();
        let available: Vec<_> = all_tasks
            .into_iter()
            .filter(|t| !linked_ids.contains(&t.id))
            .collect();

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.linking_task_mode = false;
                self.link_task_cursor = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.link_task_cursor < available.len().saturating_sub(1) {
                    self.link_task_cursor += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.link_task_cursor > 0 {
                    self.link_task_cursor -= 1;
                }
            }
            KeyCode::Enter => {
                if let Some(task) = available.get(self.link_task_cursor) {
                    self.do_link_task(task.id).await;
                }
                self.linking_task_mode = false;
                self.link_task_cursor = 0;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn do_link_task(&mut self, task_id: uuid::Uuid) {
        let workspace_id = match self.current_workspace {
            Some(ref ws) => ws.id,
            None => return,
        };
        let doc_id = match self.kb_selected_doc {
            Some(ref d) => d.id,
            None => return,
        };

        match self.api.link_task_to_document(workspace_id, doc_id, task_id).await {
            Ok(_) => {
                // Reload linked tasks
                self.load_kb_linked_tasks().await;
            }
            Err(e) => {
                self.set_error(format!("Failed to link task: {}", e));
            }
        }
    }

    async fn unlink_task_from_kb(&mut self) {
        // Just unlink the first linked task for now (could add a picker later)
        if let Some(linked_task) = self.kb_linked_tasks.first() {
            let workspace_id = match self.current_workspace {
                Some(ref ws) => ws.id,
                None => return,
            };
            let doc_id = match self.kb_selected_doc {
                Some(ref d) => d.id,
                None => return,
            };
            let task_id = linked_task.task_id;

            match self.api.unlink_task_from_document(workspace_id, doc_id, task_id).await {
                Ok(_) => {
                    self.kb_linked_tasks.retain(|t| t.task_id != task_id);
                }
                Err(e) => {
                    self.set_error(format!("Failed to unlink task: {}", e));
                }
            }
        }
    }

    async fn do_create_document(&mut self) {
        let workspace_id = match &self.current_workspace {
            Some(w) => w.id,
            None => return,
        };

        let req = CreateDocumentRequest {
            title: self.kb_create_title.clone(),
            parent_id: self.kb_create_parent_id,
            content: None,
        };

        self.set_loading(true, "Creating document...");

        match self.api.create_document(workspace_id, req).await {
            Ok(doc) => {
                // If parent was set, expand it
                if let Some(parent_id) = self.kb_create_parent_id {
                    self.kb_expanded.insert(parent_id);
                }
                self.kb_documents.push(doc.clone());
                self.build_kb_visible_list();
                // Select the new document
                if let Some(pos) = self.kb_visible_list.iter().position(|(d, _)| d.id == doc.id) {
                    self.kb_selected_idx = pos;
                    self.kb_selected_doc = Some(doc);
                }
                self.kb_creating = false;
                self.kb_create_title.clear();
                self.kb_create_parent_id = None;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to create document: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    async fn do_update_document(&mut self) {
        let workspace_id = match &self.current_workspace {
            Some(w) => w.id,
            None => return,
        };

        let doc_id = match &self.kb_selected_doc {
            Some(d) => d.id,
            None => return,
        };

        let content = self.get_kb_content();
        let req = UpdateDocumentRequest {
            title: Some(self.kb_edit_title.clone()),
            content: Some(content),
        };

        self.set_loading(true, "Updating document...");

        match self.api.update_document(workspace_id, doc_id, req).await {
            Ok(updated) => {
                // Update in local list
                if let Some(doc) = self.kb_documents.iter_mut().find(|d| d.id == doc_id) {
                    *doc = updated.clone();
                }
                self.kb_selected_doc = Some(updated);
                self.build_kb_visible_list();
                self.kb_editing = false;
                self.kb_edit_title.clear();
                self.kb_content_textarea = None;
                self.vim_mode = VimMode::Normal;
            }
            Err(e) => {
                self.set_error(format!("Failed to update document: {}", e));
            }
        }

        self.set_loading(false, "");
    }

    async fn do_delete_document(&mut self) {
        let workspace_id = match &self.current_workspace {
            Some(w) => w.id,
            None => return,
        };

        let doc_id = match &self.kb_selected_doc {
            Some(d) => d.id,
            None => return,
        };

        self.set_loading(true, "Deleting document...");

        match self.api.delete_document(workspace_id, doc_id).await {
            Ok(_) => {
                // Remove from local list (and any children)
                self.kb_documents.retain(|d| d.id != doc_id && d.parent_id != Some(doc_id));
                self.kb_expanded.remove(&doc_id);
                self.build_kb_visible_list();
                // Adjust selection
                if self.kb_selected_idx >= self.kb_visible_list.len() {
                    self.kb_selected_idx = self.kb_visible_list.len().saturating_sub(1);
                }
                self.kb_selected_doc = self.kb_visible_list.get(self.kb_selected_idx).map(|(d, _)| d.clone());
                self.kb_confirming_delete = false;
            }
            Err(e) => {
                self.set_error(format!("Failed to delete document: {}", e));
                self.kb_confirming_delete = false;
            }
        }

        self.set_loading(false, "");
    }

    async fn navigate_to_document(&mut self, doc: Document) {
        // First open knowledge base to load documents
        self.open_knowledge_base().await;

        // Find the document in the visible list
        if let Some(pos) = self.kb_visible_list.iter().position(|(d, _)| d.id == doc.id) {
            self.kb_selected_idx = pos;
            self.kb_selected_doc = Some(doc);
            self.load_kb_linked_tasks().await;
        } else {
            // Document might be under a collapsed parent - expand parents and rebuild
            // For simplicity, just select the first document and set kb_selected_doc
            self.kb_selected_doc = Some(doc);
        }
    }

    async fn load_kb_linked_tasks(&mut self) {
        let workspace_id = match &self.current_workspace {
            Some(w) => w.id,
            None => {
                self.kb_linked_tasks.clear();
                return;
            }
        };

        let doc_id = match &self.kb_selected_doc {
            Some(d) => d.id,
            None => {
                self.kb_linked_tasks.clear();
                return;
            }
        };

        match self.api.list_linked_tasks(workspace_id, doc_id).await {
            Ok(tasks) => {
                self.kb_linked_tasks = tasks;
            }
            Err(_) => {
                self.kb_linked_tasks.clear();
            }
        }
    }

    // ============ Help Modal ============

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.help_visible = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.help_scroll = self.help_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    // ============ Menu ============

    const MENU_ITEMS: [(&'static str, &'static str); 7] = [
        ("m", "Members"),
        ("k", "Knowledge Base"),
        ("t", "Tags"),
        ("f", "Filters"),
        ("p", "Presets"),
        ("/", "Search"),
        ("w", "Workspaces"),
    ];

    async fn handle_menu_key(&mut self, key: KeyEvent, tx: mpsc::Sender<AppEvent>) -> Result<bool> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.menu_visible = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if self.menu_selected_idx < Self::MENU_ITEMS.len() - 1 {
                    self.menu_selected_idx += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.menu_selected_idx = self.menu_selected_idx.saturating_sub(1);
            }
            KeyCode::Enter => {
                self.execute_menu_action(tx).await?;
            }
            // Quick select by shortcut key
            KeyCode::Char('m') => {
                self.menu_selected_idx = 0;
                self.execute_menu_action(tx).await?;
            }
            KeyCode::Char('K') => {
                self.menu_selected_idx = 1;
                self.execute_menu_action(tx).await?;
            }
            KeyCode::Char('t') => {
                self.menu_selected_idx = 2;
                self.execute_menu_action(tx).await?;
            }
            KeyCode::Char('F') => {
                self.menu_selected_idx = 3;
                self.execute_menu_action(tx).await?;
            }
            KeyCode::Char('P') => {
                self.menu_selected_idx = 4;
                self.execute_menu_action(tx).await?;
            }
            KeyCode::Char('/') => {
                self.menu_selected_idx = 5;
                self.execute_menu_action(tx).await?;
            }
            KeyCode::Char('W') => {
                self.menu_selected_idx = 6;
                self.execute_menu_action(tx).await?;
            }
            _ => {}
        }
        Ok(false)
    }

    async fn execute_menu_action(&mut self, _tx: mpsc::Sender<AppEvent>) -> Result<()> {
        self.menu_visible = false;
        match self.menu_selected_idx {
            0 => {
                // Members - load members before showing panel
                if let Some(ref workspace) = self.current_workspace {
                    if let Ok(members) = self.api.list_members(workspace.id).await {
                        self.workspace_members = members;
                    }
                }
                self.member_panel_visible = true;
                self.selected_member_idx = 0;
            }
            1 => {
                // Knowledge Base
                self.open_knowledge_base().await;
            }
            2 => {
                // Tags
                self.tag_management_visible = true;
                self.tag_management_cursor = 0;
                self.tag_management_mode = TagManagementMode::List;
            }
            3 => {
                // Filters
                self.open_filter_panel().await;
            }
            4 => {
                // Presets
                self.preset_panel_visible = true;
                self.preset_list_cursor = 0;
                self.creating_preset = false;
            }
            5 => {
                // Search
                self.searching = true;
                self.search_query.clear();
                self.search_results.clear();
                self.search_selected = 0;
                self.vim_mode = VimMode::Insert;
            }
            6 => {
                // Workspaces
                self.open_workspace_modal().await;
            }
            _ => {}
        }
        Ok(())
    }
}
