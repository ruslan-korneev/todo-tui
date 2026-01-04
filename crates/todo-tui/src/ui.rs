use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, InputField, View, VimMode};

pub fn draw(f: &mut Frame, app: &App) {
    // Draw based on current view
    match app.view {
        View::Login => draw_login(f, app),
        View::VerifyingAuth => draw_loading(f, "Verifying authentication..."),
        View::WorkspaceSelect => draw_workspace_select(f, app),
        View::Dashboard => draw_dashboard(f, app),
        View::TaskDetail => draw_dashboard(f, app), // TODO: implement task detail
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

    // Center the login form
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(12),
            Constraint::Percentage(30),
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
    let form_block = Block::default()
        .title(" Login ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = form_block.inner(form_area);
    f.render_widget(form_block, form_area);

    // Form layout
    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Email
            Constraint::Length(3), // Password
            Constraint::Length(2), // Submit hint
            Constraint::Min(0),    // Spacer
        ])
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

    // Submit hint
    let mode_text = match app.vim_mode {
        VimMode::Normal => "Press 'i' to edit, Enter to login, 'q' to quit",
        VimMode::Insert => "Type to enter, Esc for normal mode, Enter to login",
    };
    let hint = Paragraph::new(mode_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, form_chunks[2]);

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
            "j/k: select | Enter: open | q: quit",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(status, chunks[2]);
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

        let task_lines: Vec<Line> = column
            .tasks
            .iter()
            .enumerate()
            .map(|(j, task)| {
                let is_task_selected = is_selected && j == app.selected_task;
                let style = if is_task_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {} ", task.title), style))
            })
            .collect();

        let column_widget = Paragraph::new(task_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(format!(" {} ({}) ", column.status.name, column.tasks.len())),
        );

        f.render_widget(column_widget, column_chunks[i]);
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mode = match app.vim_mode {
        VimMode::Normal => "NORMAL",
        VimMode::Insert => "INSERT",
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", mode),
            Style::default().bg(Color::Blue).fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(
            "q: quit | h/l: columns | j/k: tasks",
            Style::default().fg(Color::DarkGray),
        ),
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
