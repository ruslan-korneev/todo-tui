use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, AuthMode, InputField, NewTaskField, TaskEditField, View, VimMode};
use todo_shared::Priority;

/// Returns (symbol, color) for a task's priority indicator
fn priority_indicator(priority: Option<Priority>) -> (&'static str, Color) {
    match priority {
        Some(Priority::Highest) => ("●", Color::Red),
        Some(Priority::High) => ("●", Color::Yellow),
        Some(Priority::Medium) => ("●", Color::Blue),
        Some(Priority::Low) => ("●", Color::Gray),
        Some(Priority::Lowest) => ("●", Color::DarkGray),
        None => ("○", Color::DarkGray),
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    // Draw based on current view
    match app.view {
        View::Login => draw_login(f, app),
        View::VerifyingAuth => draw_loading(f, "Verifying authentication..."),
        View::WorkspaceSelect => draw_workspace_select(f, app),
        View::Dashboard => draw_dashboard(f, app),
        View::TaskDetail => draw_task_detail(f, app),
    }

    // Draw error overlay if present
    if let Some(ref error) = app.error_message {
        draw_error_popup(f, error);
    }

    // Draw loading overlay if loading
    if app.loading {
        draw_loading_overlay(f, &app.loading_message);
    }
}

fn draw_login(f: &mut Frame, app: &App) {
    let area = f.area();

    let is_register = app.auth_mode == AuthMode::Register;
    let form_height = if is_register { 15 } else { 12 };

    // Center the login form
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(form_height),
            Constraint::Percentage(25),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(50),
            Constraint::Percentage(25),
        ])
        .split(vertical[1]);

    let form_area = horizontal[1];

    // Form container
    let title = if is_register { " Register " } else { " Login " };
    let form_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = form_block.inner(form_area);
    f.render_widget(form_block, form_area);

    // Form layout
    let constraints = if is_register {
        vec![
            Constraint::Length(3), // Email
            Constraint::Length(3), // Password
            Constraint::Length(3), // Display Name
            Constraint::Length(2), // Submit hint
            Constraint::Min(0),    // Spacer
        ]
    } else {
        vec![
            Constraint::Length(3), // Email
            Constraint::Length(3), // Password
            Constraint::Length(2), // Submit hint
            Constraint::Min(0),    // Spacer
        ]
    };

    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(inner);

    // Email field
    let email_style = if app.login_field == InputField::Email {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let email_block = Block::default()
        .title(" Email ")
        .borders(Borders::ALL)
        .border_style(email_style);
    let email_text = Paragraph::new(app.login_email.as_str()).block(email_block);
    f.render_widget(email_text, form_chunks[0]);

    // Password field
    let password_style = if app.login_field == InputField::Password {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let password_block = Block::default()
        .title(" Password ")
        .borders(Borders::ALL)
        .border_style(password_style);
    let password_display = "*".repeat(app.login_password.len());
    let password_text = Paragraph::new(password_display.as_str()).block(password_block);
    f.render_widget(password_text, form_chunks[1]);

    // Display Name field (register only)
    let (hint_idx, cursor_display_name_idx) = if is_register {
        let display_name_style = if app.login_field == InputField::DisplayName {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };
        let display_name_block = Block::default()
            .title(" Display Name ")
            .borders(Borders::ALL)
            .border_style(display_name_style);
        let display_name_text =
            Paragraph::new(app.register_display_name.as_str()).block(display_name_block);
        f.render_widget(display_name_text, form_chunks[2]);
        (3, Some(2))
    } else {
        (2, None)
    };

    // Submit hint
    let mode_text = match (app.vim_mode, is_register) {
        (VimMode::Normal, false) => "'i' edit | Enter submit | 'r' register | 'q' quit",
        (VimMode::Normal, true) => "'i' edit | Enter submit | 'l' login | 'q' quit",
        (VimMode::Insert, false) => "Type to enter | Esc normal | Enter submit",
        (VimMode::Insert, true) => "Type to enter | Esc normal | Enter submit",
    };
    let hint = Paragraph::new(mode_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, form_chunks[hint_idx]);

    // Set cursor position in insert mode
    if app.vim_mode == VimMode::Insert {
        let (x, y) = match app.login_field {
            InputField::Email => (
                form_chunks[0].x + 1 + app.login_email.len() as u16,
                form_chunks[0].y + 1,
            ),
            InputField::Password => (
                form_chunks[1].x + 1 + app.login_password.len() as u16,
                form_chunks[1].y + 1,
            ),
            InputField::DisplayName => {
                if let Some(idx) = cursor_display_name_idx {
                    (
                        form_chunks[idx].x + 1 + app.register_display_name.len() as u16,
                        form_chunks[idx].y + 1,
                    )
                } else {
                    (form_chunks[0].x + 1, form_chunks[0].y + 1)
                }
            }
        };
        f.set_cursor_position((x, y));
    }
}

fn draw_workspace_select(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // List
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    // Header
    let user_name = app
        .user
        .as_ref()
        .map(|u| u.display_name.as_str())
        .unwrap_or("Unknown");

    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "TODO TUI",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(user_name, Style::default().fg(Color::Yellow)),
    ])])
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, chunks[0]);

    // Workspace list
    let items: Vec<ListItem> = app
        .workspaces
        .iter()
        .enumerate()
        .map(|(i, ws)| {
            let style = if i == app.selected_workspace_idx {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            let role = format!("{:?}", ws.role).to_lowercase();
            let line = Line::from(vec![
                Span::raw("  "),
                Span::styled(&ws.workspace.name, style),
                Span::raw("  "),
                Span::styled(format!("({})", role), Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(" Select Workspace ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(list, chunks[1]);

    // Status bar
    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            " NORMAL ",
            Style::default().bg(Color::Blue).fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(
            "n: new | j/k: select | Enter: open | L: logout | q: quit",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(status, chunks[2]);

    // Draw workspace creation popup if active
    if app.creating_workspace {
        draw_create_workspace_popup(f, app);
    }
}

fn draw_create_workspace_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Workspace ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Name input
            Constraint::Length(2), // Hint
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    // Name input field
    let name_block = Block::default()
        .title(" Name ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let name_text = Paragraph::new(app.new_workspace_name.as_str()).block(name_block);
    f.render_widget(name_text, chunks[0]);

    // Hint
    let hint = Paragraph::new("Enter: create | Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);

    // Set cursor position
    f.set_cursor_position((
        chunks[0].x + 1 + app.new_workspace_name.len() as u16,
        chunks[0].y + 1,
    ));
}

fn draw_dashboard(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    draw_header(f, chunks[0], app);
    draw_kanban(f, chunks[1], app);
    draw_status_bar(f, chunks[2], app);

    // Draw create task popup if active
    if app.creating_task {
        draw_create_task_popup(f, app);
    }

    // Draw delete confirmation popup if active
    if app.confirming_delete {
        draw_delete_confirm_popup(f, app);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let workspace_name = app
        .current_workspace
        .as_ref()
        .map(|w| w.name.as_str())
        .unwrap_or("No workspace");

    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled(
            "TODO TUI",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | "),
        Span::styled(workspace_name, Style::default().fg(Color::Yellow)),
    ])])
    .block(Block::default().borders(Borders::BOTTOM));

    f.render_widget(header, area);
}

fn draw_kanban(f: &mut Frame, area: Rect, app: &App) {
    if app.columns.is_empty() {
        let empty = Paragraph::new("No columns. Create a task to get started.")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Kanban Board"),
            );
        f.render_widget(empty, area);
        return;
    }

    let column_count = app.columns.len();
    let constraints: Vec<Constraint> = (0..column_count)
        .map(|_| Constraint::Percentage((100 / column_count) as u16))
        .collect();

    let column_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    for (i, column) in app.columns.iter().enumerate() {
        let is_selected = i == app.selected_column;
        let border_style = if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Calculate visible height (subtract 2 for borders)
        let visible_height = column_chunks[i].height.saturating_sub(2) as usize;
        let scroll_offset = app.column_scroll_offsets.get(i).copied().unwrap_or(0);

        // Build multi-line task cards with scrolling
        let mut task_lines: Vec<Line> = Vec::new();
        let mut lines_used = 0;

        for (j, task) in column.tasks.iter().enumerate().skip(scroll_offset) {
            // Calculate lines this task will use
            let task_height = if task.due_date.is_some() { 2 } else { 1 };

            // Stop if we'd exceed visible area
            if lines_used + task_height > visible_height {
                break;
            }

            let is_task_selected = is_selected && j == app.selected_task;
            let bg_style = if is_task_selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            // Line 1: Priority indicator + title
            let (priority_symbol, priority_color) = priority_indicator(task.priority);
            task_lines.push(Line::from(vec![
                Span::styled(" ", bg_style),
                Span::styled(priority_symbol, bg_style.fg(priority_color)),
                Span::styled(" ", bg_style),
                Span::styled(&task.title, bg_style.fg(Color::White)),
            ]));
            lines_used += 1;

            // Line 2: Due date (if set)
            if let Some(due_date) = task.due_date {
                if lines_used < visible_height {
                    let date_str = due_date.format("%b %d").to_string();
                    task_lines.push(Line::from(vec![
                        Span::styled("   ", bg_style),
                        Span::styled("📅 ", bg_style.fg(Color::DarkGray)),
                        Span::styled(date_str, bg_style.fg(Color::DarkGray)),
                    ]));
                    lines_used += 1;
                }
            }

            // Empty line between cards (separator)
            if j < column.tasks.len() - 1 && lines_used < visible_height {
                task_lines.push(Line::from(""));
                lines_used += 1;
            }
        }

        // Show scroll indicator if there are more tasks
        let has_more_above = scroll_offset > 0;
        let has_more_below = scroll_offset + lines_used / 2 < column.tasks.len().saturating_sub(1);
        let scroll_indicator = if has_more_above && has_more_below {
            " ↑↓"
        } else if has_more_above {
            " ↑"
        } else if has_more_below {
            " ↓"
        } else {
            ""
        };

        let column_widget = Paragraph::new(task_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(" {} ({}){} ", column.status.name, column.tasks.len(), scroll_indicator)),
        );

        f.render_widget(column_widget, column_chunks[i]);
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (mode, mode_color) = if app.moving_task {
        ("MOVE", Color::Magenta)
    } else if app.creating_task {
        ("CREATE", Color::Green)
    } else if app.confirming_delete {
        ("DELETE", Color::Red)
    } else {
        match app.vim_mode {
            VimMode::Normal => ("NORMAL", Color::Blue),
            VimMode::Insert => ("INSERT", Color::Green),
        }
    };

    let hints = if app.moving_task {
        "h/l: move task | Esc: cancel"
    } else if app.creating_task {
        "Tab: next field | Enter: create | Esc: cancel"
    } else if app.confirming_delete {
        "y: confirm | n/Esc: cancel"
    } else {
        "n: new | d: delete | m: move | Enter: details | q: quit"
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", mode),
            Style::default().bg(mode_color).fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
    ]));

    f.render_widget(status, area);
}

fn draw_create_task_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 30, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Task ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Description
            Constraint::Length(2), // Hint
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    // Title field
    let title_style = if app.new_task_field == NewTaskField::Title {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let title_block = Block::default()
        .title(" Title ")
        .borders(Borders::ALL)
        .border_style(title_style);
    let title_text = Paragraph::new(app.new_task_title.as_str()).block(title_block);
    f.render_widget(title_text, chunks[0]);

    // Description field
    let desc_style = if app.new_task_field == NewTaskField::Description {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let desc_block = Block::default()
        .title(" Description (optional) ")
        .borders(Borders::ALL)
        .border_style(desc_style);
    let desc_text = Paragraph::new(app.new_task_description.as_str())
        .block(desc_block)
        .wrap(Wrap { trim: false });
    f.render_widget(desc_text, chunks[1]);

    // Hint
    let hint = Paragraph::new("Tab: switch field | Enter: create | Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);

    // Set cursor position
    let (cursor_x, cursor_y) = match app.new_task_field {
        NewTaskField::Title => (
            chunks[0].x + 1 + app.new_task_title.len() as u16,
            chunks[0].y + 1,
        ),
        NewTaskField::Description => (
            chunks[1].x + 1 + app.new_task_description.len() as u16,
            chunks[1].y + 1,
        ),
    };
    f.set_cursor_position((cursor_x, cursor_y));
}

fn draw_delete_confirm_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    f.render_widget(Clear, area);

    let task_title = app
        .get_selected_task()
        .map(|t| t.title.as_str())
        .unwrap_or("Unknown");

    let block = Block::default()
        .title(" Confirm Delete ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Message
            Constraint::Length(2), // Hint
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    let message = Paragraph::new(vec![
        Line::from(Span::raw("Delete task:")),
        Line::from(Span::styled(
            format!("\"{}\"", task_title),
            Style::default().fg(Color::Yellow),
        )),
    ])
    .alignment(Alignment::Center);
    f.render_widget(message, chunks[0]);

    let hint = Paragraph::new("y: yes, delete | n: no, cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);
}

fn draw_task_detail(f: &mut Frame, app: &App) {
    let task = match &app.selected_task_detail {
        Some(t) => t,
        None => return,
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Header
    draw_header(f, chunks[0], app);

    // Check if in edit mode
    if app.editing_task {
        draw_task_edit_mode(f, chunks[1], app);
    } else {
        draw_task_view_mode(f, chunks[1], app, task);
    }

    // Status bar
    draw_task_detail_status_bar(f, chunks[2], app);
}

fn draw_task_view_mode(f: &mut Frame, area: Rect, app: &App, task: &todo_shared::Task) {
    // Main content: split into task info and comments
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Task details
            Constraint::Percentage(50), // Comments
        ])
        .split(area);

    // Task details panel
    let mut task_lines = vec![
        Line::from(vec![
            Span::styled("Title: ", Style::default().fg(Color::Cyan)),
            Span::raw(&task.title),
        ]),
        Line::from(""),
    ];

    // Description
    if let Some(ref desc) = task.description {
        task_lines.push(Line::from(Span::styled(
            "Description:",
            Style::default().fg(Color::Cyan),
        )));
        for line in desc.lines() {
            task_lines.push(Line::from(format!("  {}", line)));
        }
        task_lines.push(Line::from(""));
    }

    // Priority
    if let Some(ref priority) = task.priority {
        let priority_color = match priority {
            todo_shared::Priority::Highest => Color::Red,
            todo_shared::Priority::High => Color::LightRed,
            todo_shared::Priority::Medium => Color::Yellow,
            todo_shared::Priority::Low => Color::Green,
            todo_shared::Priority::Lowest => Color::DarkGray,
        };
        task_lines.push(Line::from(vec![
            Span::styled("Priority: ", Style::default().fg(Color::Cyan)),
            Span::styled(format!("{:?}", priority), Style::default().fg(priority_color)),
        ]));
    }

    // Due date
    if let Some(ref due_date) = task.due_date {
        task_lines.push(Line::from(vec![
            Span::styled("Due Date: ", Style::default().fg(Color::Cyan)),
            Span::raw(due_date.to_string()),
        ]));
    }

    // Time estimate
    if let Some(minutes) = task.time_estimate_minutes {
        let hours = minutes / 60;
        let mins = minutes % 60;
        let estimate = if hours > 0 {
            format!("{}h {}m", hours, mins)
        } else {
            format!("{}m", mins)
        };
        task_lines.push(Line::from(vec![
            Span::styled("Time Estimate: ", Style::default().fg(Color::Cyan)),
            Span::raw(estimate),
        ]));
    }

    // Created at
    task_lines.push(Line::from(vec![
        Span::styled("Created: ", Style::default().fg(Color::Cyan)),
        Span::raw(task.created_at.format("%Y-%m-%d %H:%M").to_string()),
    ]));

    let task_details = Paragraph::new(task_lines)
        .block(
            Block::default()
                .title(" Task Details ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(task_details, content_chunks[0]);

    // Comments panel
    let comments_area = content_chunks[1];

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Comments list
            Constraint::Length(3), // Comment input (if adding)
        ])
        .split(comments_area);

    // Comments list
    let comment_items: Vec<ListItem> = app
        .task_comments
        .iter()
        .map(|comment| {
            let timestamp = comment.created_at.format("%Y-%m-%d %H:%M").to_string();
            let content = Line::from(vec![
                Span::styled(
                    format!("[{}] ", timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(&comment.content),
            ]);
            ListItem::new(content)
        })
        .collect();

    let comments_list = List::new(comment_items).block(
        Block::default()
            .title(format!(" Comments ({}) ", app.task_comments.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(comments_list, inner_chunks[0]);

    // Comment input (if adding)
    if app.adding_comment {
        let input_block = Block::default()
            .title(" New Comment ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let input = Paragraph::new(app.new_comment_content.as_str()).block(input_block);
        f.render_widget(input, inner_chunks[1]);

        // Set cursor position
        f.set_cursor_position((
            inner_chunks[1].x + 1 + app.new_comment_content.len() as u16,
            inner_chunks[1].y + 1,
        ));
    }
}

fn draw_task_edit_mode(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Edit Task ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Length(5), // Description
            Constraint::Length(3), // Priority
            Constraint::Length(3), // Due Date
            Constraint::Length(3), // Time Estimate
            Constraint::Length(3), // Assignee
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    // Helper to get field style
    let field_style = |field: TaskEditField| -> Style {
        if app.edit_field == field {
            if app.vim_mode == VimMode::Insert {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Yellow)
            }
        } else {
            Style::default().fg(Color::Gray)
        }
    };

    // Title field
    let title_block = Block::default()
        .title(" Title ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::Title));
    let title_text = Paragraph::new(app.edit_task_title.as_str()).block(title_block);
    f.render_widget(title_text, chunks[0]);

    // Description field
    let desc_block = Block::default()
        .title(" Description ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::Description));
    let desc_text = Paragraph::new(app.edit_task_description.as_str())
        .block(desc_block)
        .wrap(Wrap { trim: false });
    f.render_widget(desc_text, chunks[1]);

    // Priority field
    let priority_str = match app.edit_task_priority {
        Some(todo_shared::Priority::Highest) => "Highest",
        Some(todo_shared::Priority::High) => "High",
        Some(todo_shared::Priority::Medium) => "Medium",
        Some(todo_shared::Priority::Low) => "Low",
        Some(todo_shared::Priority::Lowest) => "Lowest",
        None => "(none)",
    };
    let priority_block = Block::default()
        .title(" Priority (h/l to change) ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::Priority));
    let priority_text = Paragraph::new(priority_str).block(priority_block);
    f.render_widget(priority_text, chunks[2]);

    // Due Date field
    let due_date_block = Block::default()
        .title(" Due Date (YYYY-MM-DD) ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::DueDate));
    let due_date_text = Paragraph::new(app.edit_task_due_date_str.as_str()).block(due_date_block);
    f.render_widget(due_date_text, chunks[3]);

    // Time Estimate field
    let time_block = Block::default()
        .title(" Time Estimate (minutes) ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::TimeEstimate));
    let time_text = Paragraph::new(app.edit_task_time_estimate_str.as_str()).block(time_block);
    f.render_widget(time_text, chunks[4]);

    // Assignee field
    let assignee_str = match app.edit_task_assignee {
        Some(id) => app
            .workspace_members
            .iter()
            .find(|m| m.user_id == id)
            .map(|m| m.display_name.as_str())
            .unwrap_or("Unknown"),
        None => "(none)",
    };
    let assignee_block = Block::default()
        .title(" Assignee (h/l to change) ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::Assignee));
    let assignee_text = Paragraph::new(assignee_str).block(assignee_block);
    f.render_widget(assignee_text, chunks[5]);

    // Set cursor position if in insert mode
    if app.vim_mode == VimMode::Insert {
        let (cursor_x, cursor_y) = match app.edit_field {
            TaskEditField::Title => (
                chunks[0].x + 1 + app.edit_task_title.len() as u16,
                chunks[0].y + 1,
            ),
            TaskEditField::Description => (
                chunks[1].x + 1 + app.edit_task_description.len() as u16,
                chunks[1].y + 1,
            ),
            TaskEditField::Priority => (chunks[2].x + 1, chunks[2].y + 1),
            TaskEditField::DueDate => (
                chunks[3].x + 1 + app.edit_task_due_date_str.len() as u16,
                chunks[3].y + 1,
            ),
            TaskEditField::TimeEstimate => (
                chunks[4].x + 1 + app.edit_task_time_estimate_str.len() as u16,
                chunks[4].y + 1,
            ),
            TaskEditField::Assignee => (chunks[5].x + 1, chunks[5].y + 1),
        };
        f.set_cursor_position((cursor_x, cursor_y));
    }
}

fn draw_task_detail_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (mode, mode_color) = if app.editing_task {
        ("EDIT", Color::Yellow)
    } else {
        match app.vim_mode {
            VimMode::Normal => ("NORMAL", Color::Blue),
            VimMode::Insert => ("INSERT", Color::Green),
        }
    };

    let hints = if app.editing_task {
        if app.vim_mode == VimMode::Insert {
            "Type to edit | Esc: normal mode"
        } else {
            "j/k: fields | i: edit | h/l: priority | Enter: save | q: cancel"
        }
    } else if app.adding_comment {
        "Type comment | Enter: submit | Esc: cancel"
    } else {
        "e: edit | a: comment | q/Esc: back"
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", mode),
            Style::default().bg(mode_color).fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(hints, Style::default().fg(Color::DarkGray)),
    ]));

    f.render_widget(status, area);
}

fn draw_loading(f: &mut Frame, message: &str) {
    let area = f.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    f.render_widget(block, area);

    let text = Paragraph::new(message)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center);

    let center = centered_rect(50, 20, area);
    f.render_widget(text, center);
}

fn draw_loading_overlay(f: &mut Frame, message: &str) {
    let area = centered_rect(40, 10, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Loading ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let text = Paragraph::new(message)
        .style(Style::default().fg(Color::Yellow))
        .alignment(Alignment::Center)
        .block(block);

    f.render_widget(text, area);
}

fn draw_error_popup(f: &mut Frame, error: &str) {
    let area = centered_rect(60, 20, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Error ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));

    let text = Paragraph::new(error)
        .style(Style::default().fg(Color::Red))
        .wrap(Wrap { trim: true })
        .block(block);

    f.render_widget(text, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
