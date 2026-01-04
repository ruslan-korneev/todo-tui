use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
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
        Span::styled("TODO TUI", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled(workspace_name, Style::default().fg(Color::Yellow)),
    ])])
    .block(Block::default().borders(Borders::BOTTOM));

    f.render_widget(header, area);
}

fn draw_kanban(f: &mut Frame, area: Rect, app: &App) {
    if app.columns.is_empty() {
        let empty = Paragraph::new("No columns. Press 'n' to create a task.")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::ALL).title("Kanban Board"));
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
                .title(column.status.name.clone()),
        );

        f.render_widget(column_widget, column_chunks[i]);
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let mode = match app.vim_mode {
        crate::app::VimMode::Normal => "NORMAL",
        crate::app::VimMode::Insert => "INSERT",
        crate::app::VimMode::Command => "COMMAND",
    };

    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            format!(" {} ", mode),
            Style::default().bg(Color::Blue).fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled("q: quit | h/l: columns | j/k: tasks", Style::default().fg(Color::DarkGray)),
    ]));

    f.render_widget(status, area);
}
