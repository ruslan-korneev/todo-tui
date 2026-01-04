use todo_shared::{Task, TaskStatus, Workspace};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Login,
    Dashboard,
    TaskDetail,
    KnowledgeBase,
    Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VimMode {
    Normal,
    Insert,
    Command,
}

pub struct App {
    pub view: View,
    pub vim_mode: VimMode,
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
    pub fn new() -> Self {
        // Demo data for initial UI testing
        Self {
            view: View::Dashboard,
            vim_mode: VimMode::Normal,
            current_workspace: None,
            columns: vec![],
            selected_column: 0,
            selected_task: 0,
        }
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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
