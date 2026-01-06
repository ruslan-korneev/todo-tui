use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, AuthMode, DueDateMode, FilterPanelSection, InputField, KbFocus, NewTaskField, TaskEditField, View, VimMode, SORT_FIELDS};
use crate::markdown;
use todo_shared::api::SearchResultItem;
use todo_shared::Priority;

/// Parse a hex color string like "#ff0000" to a ratatui Color
fn parse_hex_color(hex: &str) -> Option<Color> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }

    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;

    Some(Color::Rgb(r, g, b))
}

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

/// Wraps text to fit within a given width, respecting word boundaries
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();

    // First split by newlines to preserve intentional line breaks
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }

        let words: Vec<&str> = paragraph.split_whitespace().collect();
        if words.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut current_line = String::new();

        for word in words {
            if current_line.is_empty() {
                // First word on line - add it even if it exceeds max_width
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= max_width {
                // Word fits on current line
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                // Word doesn't fit - start new line
                lines.push(current_line);
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

pub fn draw(f: &mut Frame, app: &App) {
    // Draw based on current view
    match app.view {
        View::Login => draw_login(f, app),
        View::EmailVerification => draw_email_verification(f, app),
        View::VerifyingAuth => draw_loading(f, "Verifying authentication..."),
        View::WorkspaceSelect => draw_workspace_select(f, app),
        View::Home => draw_home(f, app),
        View::Dashboard => draw_dashboard(f, app),
        View::TaskDetail => draw_task_detail(f, app),
        View::KnowledgeBase => draw_knowledge_base(f, app),
    }

    // Draw workspace modal (can appear over Dashboard or KnowledgeBase)
    if app.workspace_modal_visible {
        draw_workspace_modal(f, app);
    }

    // Draw error overlay if present
    if let Some(ref error) = app.error_message {
        draw_error_popup(f, error);
    }

    // Draw loading overlay if loading
    if app.loading {
        draw_loading_overlay(f, &app.loading_message);
    }

    // Draw help modal (global overlay, always on top)
    if app.help_visible {
        draw_help(f, app);
    }
}

fn draw_login(f: &mut Frame, app: &App) {
    let area = f.area();

    let is_register = app.auth_mode == AuthMode::Register;
    let form_height = if is_register { 18 } else { 12 }; // 4 fields + hint for register

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

    // Form layout - Register: Username, Email, Password, DisplayName
    let constraints = if is_register {
        vec![
            Constraint::Length(3), // Username
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

    if is_register {
        // Username field
        let username_style = if app.login_field == InputField::Username {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };
        let username_block = Block::default()
            .title(" Username ")
            .borders(Borders::ALL)
            .border_style(username_style);
        let username_text = Paragraph::new(app.register_username.as_str()).block(username_block);
        f.render_widget(username_text, form_chunks[0]);

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
        f.render_widget(email_text, form_chunks[1]);

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
        f.render_widget(password_text, form_chunks[2]);

        // Display Name field
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
        f.render_widget(display_name_text, form_chunks[3]);

        // Submit hint
        let mode_text = match app.vim_mode {
            VimMode::Normal => "'i' edit | Enter submit | 'l' login | 'q' quit",
            VimMode::Insert => "Type to enter | Esc normal | Enter submit",
        };
        let hint = Paragraph::new(mode_text)
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(hint, form_chunks[4]);

        // Set cursor position in insert mode
        if app.vim_mode == VimMode::Insert {
            let (x, y) = match app.login_field {
                InputField::Username => (
                    form_chunks[0].x + 1 + app.register_username.len() as u16,
                    form_chunks[0].y + 1,
                ),
                InputField::Email => (
                    form_chunks[1].x + 1 + app.login_email.len() as u16,
                    form_chunks[1].y + 1,
                ),
                InputField::Password => (
                    form_chunks[2].x + 1 + app.login_password.len() as u16,
                    form_chunks[2].y + 1,
                ),
                InputField::DisplayName => (
                    form_chunks[3].x + 1 + app.register_display_name.len() as u16,
                    form_chunks[3].y + 1,
                ),
                InputField::VerificationCode => (form_chunks[0].x + 1, form_chunks[0].y + 1),
            };
            f.set_cursor_position((x, y));
        }
    } else {
        // Login mode - Email and Password only
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
            VimMode::Normal => "'i' edit | Enter submit | 'r' register | 'q' quit",
            VimMode::Insert => "Type to enter | Esc normal | Enter submit",
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
                _ => (form_chunks[0].x + 1, form_chunks[0].y + 1),
            };
            f.set_cursor_position((x, y));
        }
    }
}

fn draw_email_verification(f: &mut Frame, app: &App) {
    let area = f.area();

    // Center the verification form
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
        .title(" Email Verification ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = form_block.inner(form_area);
    f.render_widget(form_block, form_area);

    let form_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(2), // Info text
            Constraint::Length(3), // Code input
            Constraint::Length(2), // Hint
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    // Info text
    let info_text = format!("Verification code sent to {}", app.verification_email);
    let info = Paragraph::new(info_text)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center);
    f.render_widget(info, form_chunks[0]);

    // Code input field
    let code_style = Style::default().fg(Color::Yellow);
    let code_block = Block::default()
        .title(" 6-Digit Code ")
        .borders(Borders::ALL)
        .border_style(code_style);
    let code_text = Paragraph::new(app.verification_code.as_str())
        .block(code_block)
        .alignment(Alignment::Center);
    f.render_widget(code_text, form_chunks[1]);

    // Hint
    let mode_text = match app.vim_mode {
        VimMode::Normal => "'i' edit | Enter verify | 'r' resend | Esc back",
        VimMode::Insert => "Type code | Esc normal | Enter verify",
    };
    let hint = Paragraph::new(mode_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, form_chunks[2]);

    // Set cursor position in insert mode
    if app.vim_mode == VimMode::Insert {
        let x = form_chunks[1].x + 1 + form_chunks[1].width / 2 - 3 + app.verification_code.len() as u16;
        let y = form_chunks[1].y + 1;
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
            "n: new | i: join invite | j/k: select | Enter: open | L: logout | q: quit",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(status, chunks[2]);

    // Draw workspace creation popup if active
    if app.creating_workspace {
        draw_create_workspace_popup(f, app);
    }

    // Draw invite acceptance popup if active
    if app.accepting_invite {
        draw_accept_invite_popup(f, app);
    }
}

fn draw_home(f: &mut Frame, app: &App) {
    let area = f.area();

    // Main layout: left menu/stats, right content (logo + quote)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left panel: menu + stats
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(8),     // Menu
            Constraint::Length(10), // Stats
            Constraint::Length(1),  // Status bar
        ])
        .split(main_chunks[0]);

    // Right panel: logo + calendar + quote
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Logo
            Constraint::Length(10), // Calendar
            Constraint::Min(3),     // Quote
        ])
        .split(main_chunks[1]);

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
    f.render_widget(header, left_chunks[0]);

    // Menu
    draw_home_menu(f, left_chunks[1], app);

    // Stats
    draw_home_stats(f, left_chunks[2], app);

    // Status bar
    let status = Paragraph::new(Line::from(vec![
        Span::styled(
            " HOME ",
            Style::default().bg(Color::Magenta).fg(Color::White),
        ),
        Span::raw(" "),
        Span::styled(
            "j/k: navigate | Enter: select | Ctrl+W: workspaces | q: quit",
            Style::default().fg(Color::DarkGray),
        ),
    ]));
    f.render_widget(status, left_chunks[3]);

    // Logo (figlet workspace name)
    draw_home_logo(f, right_chunks[0], app);

    // Calendar
    draw_home_calendar(f, right_chunks[1], app);

    // Quote
    draw_home_quote(f, right_chunks[2], app);
}

fn draw_home_menu(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Navigation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = crate::app::HomeMenuItem::all()
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == app.home_menu_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let indicator = if is_selected { "▸ " } else { "  " };

            ListItem::new(Line::from(vec![
                Span::styled(indicator, style),
                Span::styled(item.icon(), style),
                Span::raw(" "),
                Span::styled(item.label(), style),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn draw_home_stats(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Workspace Stats ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let stats = &app.home_stats;
    let lines = vec![
        Line::from(vec![
            Span::styled("📅 ", Style::default()),
            Span::styled(
                format!("{} tasks due today", stats.tasks_due_today),
                if stats.tasks_due_today > 0 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("⚠️  ", Style::default()),
            Span::styled(
                format!("{} overdue", stats.overdue_tasks),
                if stats.overdue_tasks > 0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
        ]),
        Line::from(vec![
            Span::styled("📋 ", Style::default()),
            Span::styled(
                format!("{} pending tasks", stats.pending_tasks),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("✅ ", Style::default()),
            Span::styled(
                format!("{} completed this week", stats.completed_this_week),
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("👤 ", Style::default()),
            Span::styled(
                format!("{} assigned to me", stats.assigned_to_me),
                Style::default().fg(Color::Blue),
            ),
        ]),
        Line::from(vec![
            Span::styled("📚 ", Style::default()),
            Span::styled(
                format!("{} documents", stats.documents_count),
                Style::default().fg(Color::Magenta),
            ),
        ]),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, inner);
}

fn draw_home_logo(f: &mut Frame, area: Rect, app: &App) {
    let workspace_name = app
        .current_workspace
        .as_ref()
        .map(|w| w.name.as_str())
        .unwrap_or("TODO");

    let ascii_lines = crate::figlet::render(workspace_name);

    // Calculate the width of the ASCII art (max line length)
    let ascii_width = ascii_lines.iter().map(|s| s.len()).max().unwrap_or(0) as u16;

    // Center the ASCII art block horizontally
    let x_offset = area.x + area.width.saturating_sub(ascii_width) / 2;
    let centered_area = Rect::new(x_offset, area.y, ascii_width.min(area.width), area.height);

    let lines: Vec<Line> = ascii_lines
        .into_iter()
        .map(|s| {
            Line::from(Span::styled(
                s,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ))
        })
        .collect();

    // Use left alignment to preserve ASCII art internal alignment
    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, centered_area);
}

fn draw_home_calendar(f: &mut Frame, area: Rect, app: &App) {
    let title = format!(
        " {} {} ",
        crate::calendar::month_name(app.calendar_month),
        app.calendar_year
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let today = chrono::Local::now().date_naive();
    let lines = crate::calendar::render_calendar(
        app.calendar_year,
        app.calendar_month,
        &app.calendar_tasks,
        today,
    );

    let paragraph = Paragraph::new(lines);
    f.render_widget(paragraph, inner);
}

fn draw_home_quote(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Quote of the Day ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let quote = app.home_quote.as_deref().unwrap_or("Loading...");
    let author = app.home_quote_author.as_deref().unwrap_or("");

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("\"{}\"", quote),
            Style::default().fg(Color::White).add_modifier(Modifier::ITALIC),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("— {}", author),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
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

fn draw_accept_invite_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 25, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Accept Invitation ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Token input
            Constraint::Length(2), // Hint
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    // Token input field
    let token_block = Block::default()
        .title(" Invite Token ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let token_text = Paragraph::new(app.invite_token_input.as_str()).block(token_block);
    f.render_widget(token_text, chunks[0]);

    // Hint
    let hint = Paragraph::new("Paste the invite token you received | Enter: accept | Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);

    // Set cursor position
    f.set_cursor_position((
        chunks[0].x + 1 + app.invite_token_input.len() as u16,
        chunks[0].y + 1,
    ));
}

fn draw_dashboard(f: &mut Frame, app: &App) {
    let filter_bar_height = if app.filter_bar_visible { 2 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),                      // Header
            Constraint::Length(filter_bar_height),      // Filter bar (optional)
            Constraint::Min(0),                         // Main content
            Constraint::Length(1),                      // Status bar
        ])
        .split(f.area());

    draw_header(f, chunks[0], app);

    if app.filter_bar_visible {
        draw_filter_bar(f, chunks[1], app);
    }

    draw_kanban(f, chunks[2], app);

    // Draw command input at the bottom if in command mode
    if app.command_mode {
        draw_command_input(f, chunks[3], app);
    } else {
        draw_status_bar(f, chunks[3], app);
    }

    // Draw create task popup if active
    if app.creating_task {
        draw_create_task_popup(f, app);
    }

    // Draw delete confirmation popup if active
    if app.confirming_delete {
        draw_delete_confirm_popup(f, app);
    }

    // Draw search popup if active
    if app.searching {
        draw_search_popup(f, app);
    }

    // Draw tag management popup if active
    if app.tag_management_visible {
        draw_tag_management_popup(f, app);
    }

    // Draw member panel popup if active
    if app.member_panel_visible {
        draw_member_panel(f, app);
    }

    // Draw filter panel popup if active
    if app.filter_panel_visible {
        draw_filter_panel(f, app);
    }

    // Draw preset panel popup if active
    if app.preset_panel_visible {
        draw_preset_panel(f, app);
    }

    // Draw menu popup if active
    if app.menu_visible {
        draw_menu(f, app);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let workspace_name = app
        .current_workspace
        .as_ref()
        .map(|w| w.name.as_str())
        .unwrap_or("No workspace");

    let username = app
        .user
        .as_ref()
        .map(|u| format!("@{}", u.username))
        .unwrap_or_default();

    let (status_icon, status_text, status_color) = if app.is_connected {
        ("●", "Connected", Color::Green)
    } else {
        ("○", "Disconnected", Color::Red)
    };

    // Calculate right side content length
    let right_content = format!("{}  {} {}", username, status_icon, status_text);
    let left_len = workspace_name.len() + 1; // +1 for leading space
    let right_len = right_content.len() + 1; // +1 for trailing space
    let available_width = area.width.saturating_sub(2) as usize; // -2 for borders
    let padding = available_width.saturating_sub(left_len + right_len);

    let header = Paragraph::new(vec![Line::from(vec![
        Span::raw(" "),
        Span::styled(
            workspace_name,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(padding)),
        Span::styled(&username, Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(status_icon, Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(status_text, Style::default().fg(status_color)),
        Span::raw(" "),
    ])])
    .block(Block::default().borders(Borders::BOTTOM));

    f.render_widget(header, area);
}

fn draw_filter_bar(f: &mut Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span> = vec![Span::styled(
        " Filters: ",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];

    let filter_style = Style::default()
        .bg(Color::DarkGray)
        .fg(Color::White);

    let mut has_filters = false;

    // Text search filter
    if let Some(ref q) = app.active_filters.q {
        if !q.is_empty() {
            spans.push(Span::styled(format!(" q: \"{}\" ", q), filter_style));
            spans.push(Span::raw(" "));
            has_filters = true;
        }
    }

    // Priority filter
    if let Some(priority) = &app.active_filters.priority {
        let priority_str = match priority {
            Priority::Highest => "Highest",
            Priority::High => "High",
            Priority::Medium => "Medium",
            Priority::Low => "Low",
            Priority::Lowest => "Lowest",
        };
        spans.push(Span::styled(format!(" Priority: {} ", priority_str), filter_style));
        spans.push(Span::raw(" "));
        has_filters = true;
    }

    // Tags filter
    if let Some(ref tag_ids) = app.active_filters.tag_ids {
        if !tag_ids.is_empty() {
            // Get tag names for display
            let tag_names: Vec<&str> = tag_ids
                .iter()
                .filter_map(|id| app.workspace_tags.iter().find(|t| &t.id == id))
                .map(|t| t.name.as_str())
                .take(2)
                .collect();
            let remaining = tag_ids.len().saturating_sub(2);

            let tag_display = if remaining > 0 {
                format!(" Tags: {} +{} ", tag_names.join(", "), remaining)
            } else {
                format!(" Tags: {} ", tag_names.join(", "))
            };
            spans.push(Span::styled(tag_display, Style::default().bg(Color::Magenta).fg(Color::White)));
            spans.push(Span::raw(" "));
            has_filters = true;
        }
    }

    // Assignee filter
    if let Some(assignee_id) = &app.active_filters.assigned_to {
        let assignee_name = app
            .workspace_members
            .iter()
            .find(|m| &m.user_id == assignee_id)
            .map(|m| m.display_name.as_str())
            .unwrap_or("Unknown");
        spans.push(Span::styled(format!(" Assignee: {} ", assignee_name), filter_style));
        spans.push(Span::raw(" "));
        has_filters = true;
    }

    // Due date filters
    if let Some(date) = &app.active_filters.due_before {
        spans.push(Span::styled(format!(" Due <{} ", date), filter_style));
        spans.push(Span::raw(" "));
        has_filters = true;
    }

    if let Some(date) = &app.active_filters.due_after {
        spans.push(Span::styled(format!(" Due >{} ", date), filter_style));
        spans.push(Span::raw(" "));
        has_filters = true;
    }

    // Sort indicator
    if let Some(order_by) = &app.active_filters.order_by {
        let direction = app
            .active_filters
            .order
            .as_ref()
            .map(|o| if o == "DESC" { "↑" } else { "↓" })
            .unwrap_or("");
        spans.push(Span::styled(
            format!(" Sort: {} {} ", order_by, direction),
            Style::default().bg(Color::Blue).fg(Color::White),
        ));
        has_filters = true;
    }

    // Show hint if no filters active but bar is visible
    if !has_filters {
        spans.push(Span::styled(
            "None ",
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Add keyboard hints
    spans.push(Span::styled(
        "│ F: panel  f: hide  :clear",
        Style::default().fg(Color::DarkGray),
    ));

    let filter_bar = Paragraph::new(Line::from(spans))
        .style(Style::default().bg(Color::Black));

    f.render_widget(filter_bar, area);
}

fn draw_command_input(f: &mut Frame, area: Rect, app: &App) {
    let command_line = Paragraph::new(Line::from(vec![
        Span::styled(":", Style::default().fg(Color::Yellow)),
        Span::raw(&app.command_input),
    ]))
    .style(Style::default().bg(Color::Black));

    f.render_widget(command_line, area);

    // Set cursor position
    f.set_cursor_position((
        area.x + 1 + app.command_input.len() as u16,
        area.y,
    ));
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
        let column_border_style = if is_selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let scroll_offset = app.column_scroll_offsets.get(i).copied().unwrap_or(0);

        // Calculate scroll indicators
        let has_more_above = scroll_offset > 0;
        let visible_tasks_estimate = (column_chunks[i].height.saturating_sub(2) / 4) as usize;
        let has_more_below = scroll_offset + visible_tasks_estimate < column.tasks.len();
        let scroll_indicator = if has_more_above && has_more_below {
            " ↑↓"
        } else if has_more_above {
            " ↑"
        } else if has_more_below {
            " ↓"
        } else {
            ""
        };

        // Render column block first
        let column_block = Block::default()
            .borders(Borders::ALL)
            .border_style(column_border_style)
            .title(format!(
                " {} ({}){}",
                column.status.name,
                column.tasks.len(),
                scroll_indicator
            ));
        let inner_area = column_block.inner(column_chunks[i]);
        f.render_widget(column_block, column_chunks[i]);

        // Render each task card with its own border
        let mut y_offset: u16 = 0;
        for (j, task) in column.tasks.iter().enumerate().skip(scroll_offset) {
            // Calculate task card height: 1 line for title, +1 if due date, +1 if tags, +2 for borders
            let content_lines = 1
                + if task.due_date.is_some() { 1 } else { 0 }
                + if !task.tags.is_empty() { 1 } else { 0 };
            let card_height = (content_lines + 2) as u16; // +2 for top/bottom borders

            // Stop if we'd exceed visible area
            if y_offset + card_height > inner_area.height {
                break;
            }

            let is_task_selected = is_selected && j == app.selected_task;
            let task_border_style = if is_task_selected {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Calculate task card area
            let task_area = Rect {
                x: inner_area.x,
                y: inner_area.y + y_offset,
                width: inner_area.width,
                height: card_height,
            };

            // Build task content lines
            let mut task_content: Vec<Line> = Vec::new();

            // Line 1: Priority indicator + title (with search highlighting if filter active)
            let (priority_symbol, priority_color) = priority_indicator(task.priority);
            let title_spans = if let Some(ref query) = app.active_filters.q {
                let mut spans = vec![
                    Span::styled(priority_symbol, Style::default().fg(priority_color)),
                    Span::styled(" ", Style::default()),
                ];
                spans.extend(highlight_search_matches(&task.title, query, Style::default().fg(Color::White)));
                spans
            } else {
                vec![
                    Span::styled(priority_symbol, Style::default().fg(priority_color)),
                    Span::styled(" ", Style::default()),
                    Span::styled(task.title.clone(), Style::default().fg(Color::White)),
                ]
            };
            task_content.push(Line::from(title_spans));

            // Line 2: Due date (if set)
            if let Some(due_date) = task.due_date {
                let date_str = due_date.format("%b %d").to_string();
                task_content.push(Line::from(vec![
                    Span::styled("📅 ", Style::default().fg(Color::DarkGray)),
                    Span::styled(date_str, Style::default().fg(Color::DarkGray)),
                ]));
            }

            // Line 3: Tags (if any)
            if !task.tags.is_empty() {
                let mut tag_spans: Vec<Span> = Vec::new();

                // Show up to 2 tags, then "+N" for more
                let display_tags = task.tags.iter().take(2);
                let remaining = task.tags.len().saturating_sub(2);

                for (idx, tag) in display_tags.enumerate() {
                    if idx > 0 {
                        tag_spans.push(Span::styled(" ", Style::default()));
                    }
                    // Parse tag color or use default
                    let tag_color = tag
                        .color
                        .as_ref()
                        .and_then(|c| parse_hex_color(c))
                        .unwrap_or(Color::Gray);
                    tag_spans.push(Span::styled(
                        format!(" {} ", tag.name),
                        Style::default().bg(tag_color).fg(Color::Black),
                    ));
                }

                if remaining > 0 {
                    tag_spans.push(Span::styled(
                        format!(" +{}", remaining),
                        Style::default().fg(Color::DarkGray),
                    ));
                }

                task_content.push(Line::from(tag_spans));
            }

            // Render task card with border
            let task_block = Block::default()
                .borders(Borders::ALL)
                .border_style(task_border_style);

            let task_widget = Paragraph::new(task_content).block(task_block);
            f.render_widget(task_widget, task_area);

            y_offset += card_height;
        }
    }
}

fn draw_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (mode, mode_color) = if app.searching {
        ("SEARCH", Color::Cyan)
    } else if app.moving_task {
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

    let hints = if app.searching {
        "Type to search | Enter: select | Ctrl+F: fuzzy | Esc: cancel"
    } else if app.moving_task {
        "h/l: move task | Esc: cancel"
    } else if app.creating_task {
        "Tab: next field | Enter: create | Esc: cancel"
    } else if app.confirming_delete {
        "y: confirm | n/Esc: cancel"
    } else {
        "?: help | ^P: menu | /: search | n: new | d: del | m: move | Enter: open"
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

    // Description field (uses TextArea when available)
    let desc_style = if app.new_task_field == NewTaskField::Description {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    if let Some(ref textarea) = app.new_task_description_textarea {
        // Render TextArea with custom block
        let desc_block = Block::default()
            .title(" Description (Ctrl+E: editor, Alt+Enter: create) ")
            .borders(Borders::ALL)
            .border_style(desc_style);
        let inner = desc_block.inner(chunks[1]);
        f.render_widget(desc_block, chunks[1]);
        f.render_widget(textarea, inner);
    } else {
        let desc_block = Block::default()
            .title(" Description (optional) ")
            .borders(Borders::ALL)
            .border_style(desc_style);
        let desc_text = Paragraph::new("").block(desc_block);
        f.render_widget(desc_text, chunks[1]);
    }

    // Hint
    let hint = Paragraph::new("Tab: switch field | Enter: create | Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);

    // Set cursor position (only for title field, textarea handles its own cursor)
    if app.new_task_field == NewTaskField::Title {
        let cursor_x = chunks[0].x + 1 + app.new_task_title.len() as u16;
        let cursor_y = chunks[0].y + 1;
        f.set_cursor_position((cursor_x, cursor_y));
    }
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

fn draw_search_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(70, 60, f.area());

    f.render_widget(Clear, area);

    let title = if app.search_fuzzy {
        " Search (fuzzy) "
    } else {
        " Search "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(0),    // Results list
            Constraint::Length(2), // Hints
        ])
        .split(inner);

    // Search input
    let input_block = Block::default()
        .title(" Query ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let input = Paragraph::new(app.search_query.as_str()).block(input_block);
    f.render_widget(input, chunks[0]);

    // Results list
    let result_items: Vec<ListItem> = app
        .search_results
        .iter()
        .enumerate()
        .map(|(i, result)| {
            let is_selected = i == app.search_selected;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            match result {
                SearchResultItem::Task(task_result) => {
                    let (priority_symbol, priority_color) = priority_indicator(task_result.task.priority);

                    // Build the line with highlighted title
                    let mut spans = vec![
                        Span::styled("  ", style),
                        Span::styled(priority_symbol, style.fg(priority_color)),
                        Span::styled(" ", style),
                    ];

                    // Parse title with highlight markers
                    let title_text = task_result
                        .title_highlights
                        .as_deref()
                        .unwrap_or(&task_result.task.title);
                    spans.extend(parse_highlights_to_spans(title_text, style));

                    // Add rank score
                    spans.push(Span::styled(
                        format!(" ({:.2})", task_result.rank),
                        style.fg(Color::DarkGray),
                    ));

                    ListItem::new(Line::from(spans))
                }
                SearchResultItem::Document(doc_result) => {
                    // Build the line with document icon and highlighted title
                    let mut spans = vec![
                        Span::styled("  ", style),
                        Span::styled("[D]", style.fg(Color::Cyan)),
                        Span::styled(" ", style),
                    ];

                    // Parse title with highlight markers
                    let title_text = doc_result
                        .title_highlights
                        .as_deref()
                        .unwrap_or(&doc_result.document.title);
                    spans.extend(parse_highlights_to_spans(title_text, style));

                    // Add path breadcrumb
                    spans.push(Span::styled(
                        format!(" ({})", doc_result.document.path),
                        style.fg(Color::DarkGray),
                    ));

                    // Add rank score
                    spans.push(Span::styled(
                        format!(" [{:.2}]", doc_result.rank),
                        style.fg(Color::DarkGray),
                    ));

                    ListItem::new(Line::from(spans))
                }
            }
        })
        .collect();

    let results_title = if app.search_results.is_empty() && !app.search_query.is_empty() {
        " No results ".to_string()
    } else {
        format!(" Results ({}) ", app.search_total)
    };

    let results_list = List::new(result_items).block(
        Block::default()
            .title(results_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(results_list, chunks[1]);

    // Hints
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(": select | "),
        Span::styled("Ctrl+F", Style::default().fg(Color::Yellow)),
        Span::raw(": toggle fuzzy | "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": cancel"),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);

    // Set cursor position
    f.set_cursor_position((
        chunks[0].x + 1 + app.search_query.len() as u16,
        chunks[0].y + 1,
    ));
}

fn draw_tag_management_popup(f: &mut Frame, app: &App) {
    use crate::app::{TagManagementMode, TAG_COLORS};

    let area = centered_rect(50, 60, f.area());
    f.render_widget(Clear, area);

    let title = match app.tag_management_mode {
        TagManagementMode::List => " Manage Tags ",
        TagManagementMode::Create => " Create Tag ",
        TagManagementMode::Edit => " Edit Tag ",
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    match app.tag_management_mode {
        TagManagementMode::List => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Min(0),    // Tag list
                    Constraint::Length(2), // Hints
                ])
                .split(inner);

            // Tag list
            let tag_items: Vec<ListItem> = app
                .workspace_tags
                .iter()
                .enumerate()
                .map(|(i, tag)| {
                    let is_selected = i == app.tag_management_cursor;
                    let style = if is_selected {
                        Style::default().bg(Color::DarkGray).fg(Color::White)
                    } else {
                        Style::default()
                    };

                    let tag_color = tag.color.as_ref()
                        .and_then(|c| parse_hex_color(c))
                        .unwrap_or(Color::Gray);

                    ListItem::new(Line::from(vec![
                        Span::styled("  ", style),
                        Span::styled(
                            format!(" {} ", tag.name),
                            Style::default().bg(tag_color).fg(Color::Black),
                        ),
                        Span::styled(
                            format!("  {}", tag.color.as_deref().unwrap_or("")),
                            style.fg(Color::DarkGray),
                        ),
                    ]))
                })
                .collect();

            let list_title = if app.workspace_tags.is_empty() {
                " No tags - press 'n' to create ".to_string()
            } else {
                format!(" Tags ({}) ", app.workspace_tags.len())
            };

            let list = List::new(tag_items).block(
                Block::default()
                    .title(list_title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Gray)),
            );
            f.render_widget(list, chunks[0]);

            // Hints
            let hint = Paragraph::new(Line::from(vec![
                Span::styled("n", Style::default().fg(Color::Yellow)),
                Span::raw(": new | "),
                Span::styled("e", Style::default().fg(Color::Yellow)),
                Span::raw(": edit | "),
                Span::styled("d", Style::default().fg(Color::Yellow)),
                Span::raw(": delete | "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(": close"),
            ]))
            .alignment(Alignment::Center);
            f.render_widget(hint, chunks[1]);
        }
        TagManagementMode::Create | TagManagementMode::Edit => {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([
                    Constraint::Length(3), // Name input
                    Constraint::Length(3), // Color selector
                    Constraint::Min(0),    // Spacer
                    Constraint::Length(2), // Hints
                ])
                .split(inner);

            // Name input
            let name_block = Block::default()
                .title(" Name ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            let name_input = Paragraph::new(app.tag_create_name.as_str()).block(name_block);
            f.render_widget(name_input, chunks[0]);

            // Color selector
            let selected_color = TAG_COLORS.get(app.tag_create_color_idx).unwrap_or(&"#6B7280");
            let color_preview = parse_hex_color(selected_color).unwrap_or(Color::Gray);

            let color_block = Block::default()
                .title(" Color (Tab to change) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray));
            let color_display = Paragraph::new(Line::from(vec![
                Span::styled(
                    "  ██  ",
                    Style::default().fg(color_preview),
                ),
                Span::raw(format!(" {} ", selected_color)),
            ]))
            .block(color_block);
            f.render_widget(color_display, chunks[1]);

            // Hints
            let hint = Paragraph::new(Line::from(vec![
                Span::styled("Enter", Style::default().fg(Color::Yellow)),
                Span::raw(": save | "),
                Span::styled("Tab", Style::default().fg(Color::Yellow)),
                Span::raw(": change color | "),
                Span::styled("Esc", Style::default().fg(Color::Yellow)),
                Span::raw(": cancel"),
            ]))
            .alignment(Alignment::Center);
            f.render_widget(hint, chunks[3]);

            // Set cursor position
            f.set_cursor_position((
                chunks[0].x + 1 + app.tag_create_name.len() as u16,
                chunks[0].y + 1,
            ));
        }
    }
}

fn draw_member_panel(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, f.area());
    f.render_widget(Clear, area);

    let title = if app.inviting_member {
        " Invite Member "
    } else {
        " Workspace Members "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.inviting_member {
        // Invite form
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Email input
                Constraint::Length(3), // Role selector
                Constraint::Min(0),    // Spacer
                Constraint::Length(2), // Hints
            ])
            .split(inner);

        // Email input
        let email_block = Block::default()
            .title(" Email ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let email_input = Paragraph::new(app.invite_email.as_str()).block(email_block);
        f.render_widget(email_input, chunks[0]);

        // Role selector
        let roles = ["Reader", "Editor", "Admin"];
        let selected_role = roles.get(app.invite_role_idx).unwrap_or(&"Reader");
        let role_block = Block::default()
            .title(" Role (Tab to change) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));
        let role_display = Paragraph::new(format!(" {} ", selected_role)).block(role_block);
        f.render_widget(role_display, chunks[1]);

        // Hints
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": send invite | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": change role | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hint, chunks[3]);

        // Set cursor position
        f.set_cursor_position((
            chunks[0].x + 1 + app.invite_email.len() as u16,
            chunks[0].y + 1,
        ));
    } else {
        // Member list
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Min(0),    // Member list
                Constraint::Length(2), // Hints
            ])
            .split(inner);

        let member_items: Vec<ListItem> = app
            .workspace_members
            .iter()
            .enumerate()
            .map(|(i, member)| {
                let is_selected = i == app.selected_member_idx;
                let style = if is_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                let role_style = match member.role {
                    todo_shared::WorkspaceRole::Owner => Style::default().fg(Color::Yellow),
                    todo_shared::WorkspaceRole::Admin => Style::default().fg(Color::Red),
                    todo_shared::WorkspaceRole::Editor => Style::default().fg(Color::Green),
                    todo_shared::WorkspaceRole::Reader => Style::default().fg(Color::Gray),
                };

                let role_str = match member.role {
                    todo_shared::WorkspaceRole::Owner => "OWNER",
                    todo_shared::WorkspaceRole::Admin => "ADMIN",
                    todo_shared::WorkspaceRole::Editor => "EDIT ",
                    todo_shared::WorkspaceRole::Reader => "READ ",
                };

                ListItem::new(Line::from(vec![
                    Span::styled("  ", style),
                    Span::styled(format!("[{}]", role_str), role_style),
                    Span::styled(format!(" {} ", member.display_name), style),
                    Span::styled(format!("({})", member.email), style.fg(Color::DarkGray)),
                ]))
            })
            .collect();

        let list_title = if app.workspace_members.is_empty() {
            " No members ".to_string()
        } else {
            format!(" Members ({}) ", app.workspace_members.len())
        };

        let list = List::new(member_items).block(
            Block::default()
                .title(list_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        );
        f.render_widget(list, chunks[0]);

        // Hints
        let hint = Paragraph::new(Line::from(vec![
            Span::styled("i", Style::default().fg(Color::Yellow)),
            Span::raw(": invite | "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(": change role | "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(": remove | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": close"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hint, chunks[1]);
    }
}

/// Highlight search query matches in text (client-side, case-insensitive)
fn highlight_search_matches(text: &str, query: &str, base_style: Style) -> Vec<Span<'static>> {
    if query.is_empty() {
        return vec![Span::styled(text.to_string(), base_style)];
    }

    let highlight_style = base_style.fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let lower_text = text.to_lowercase();
    let lower_query = query.to_lowercase();

    let mut spans = Vec::new();
    let mut last_end = 0;

    for (start, _) in lower_text.match_indices(&lower_query) {
        // Add text before match
        if start > last_end {
            spans.push(Span::styled(text[last_end..start].to_string(), base_style));
        }
        // Add highlighted match (using original case)
        let end = start + query.len();
        spans.push(Span::styled(text[start..end].to_string(), highlight_style));
        last_end = end;
    }

    // Add remaining text
    if last_end < text.len() {
        spans.push(Span::styled(text[last_end..].to_string(), base_style));
    }

    // Return at least one span if empty
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
}

/// Parse highlight markers (<< >>) into styled spans
fn parse_highlights_to_spans<'a>(text: &'a str, base_style: Style) -> Vec<Span<'a>> {
    let highlight_style = base_style.fg(Color::Yellow).add_modifier(Modifier::BOLD);

    let mut spans = Vec::new();
    let mut remaining = text;

    while let Some(start_pos) = remaining.find("<<") {
        // Add text before the marker
        if start_pos > 0 {
            spans.push(Span::styled(
                remaining[..start_pos].to_string(),
                base_style,
            ));
        }

        // Find end marker
        let after_start = &remaining[start_pos + 2..];
        if let Some(end_pos) = after_start.find(">>") {
            // Add highlighted text
            spans.push(Span::styled(
                after_start[..end_pos].to_string(),
                highlight_style,
            ));
            remaining = &after_start[end_pos + 2..];
        } else {
            // No end marker found, add rest as plain text
            spans.push(Span::styled(remaining[start_pos..].to_string(), base_style));
            remaining = "";
            break;
        }
    }

    // Add any remaining text
    if !remaining.is_empty() {
        spans.push(Span::styled(remaining.to_string(), base_style));
    }

    // Return at least one span if empty
    if spans.is_empty() {
        spans.push(Span::styled(text.to_string(), base_style));
    }

    spans
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

    // Draw link/unlink document popup if active
    if app.linking_document_mode {
        draw_link_document_popup(f, app);
    } else if app.unlinking_document_mode {
        draw_unlink_document_popup(f, app);
    }
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

    // Linked Documents section
    task_lines.push(Line::from(""));
    task_lines.push(Line::from(Span::styled(
        format!("Linked Documents ({}):", app.task_linked_documents.len()),
        Style::default().fg(Color::Cyan),
    )));
    if app.task_linked_documents.is_empty() {
        task_lines.push(Line::from(Span::styled(
            "  (none)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for doc in &app.task_linked_documents {
            task_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(&doc.document_title, Style::default().fg(Color::Yellow)),
            ]));
        }
    }

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

    // Comments list with text wrapping
    let available_width = inner_chunks[0].width.saturating_sub(2) as usize; // -2 for borders
    let comment_items: Vec<ListItem> = app
        .task_comments
        .iter()
        .map(|comment| {
            let timestamp = comment.created_at.format("%Y-%m-%d %H:%M").to_string();
            let header_prefix = format!("[{}][@{}]: ", timestamp, comment.author_username);
            let header_len = header_prefix.len();

            // Calculate available width for content (after header on first line)
            let content_width = available_width.saturating_sub(header_len);

            // Wrap the comment content
            let wrapped_lines = wrap_text(&comment.content, content_width);

            // Build multi-line ListItem
            let mut lines: Vec<Line> = Vec::new();
            for (i, line_text) in wrapped_lines.iter().enumerate() {
                if i == 0 {
                    // First line: header + content
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("[{}]", timestamp),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(
                            format!("[@{}]: ", comment.author_username),
                            Style::default().fg(Color::Cyan),
                        ),
                        Span::raw(line_text.clone()),
                    ]));
                } else {
                    // Continuation lines: indent to align with content
                    let indent = " ".repeat(header_len);
                    lines.push(Line::from(Span::raw(format!("{}{}", indent, line_text))));
                }
            }

            ListItem::new(lines)
        })
        .collect();

    let comments_list = List::new(comment_items).block(
        Block::default()
            .title(format!(" Comments ({}) ", app.task_comments.len()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(comments_list, inner_chunks[0]);

    // Comment input (if adding) - uses TextArea
    if app.adding_comment {
        if let Some(ref textarea) = app.comment_textarea {
            let input_block = Block::default()
                .title(" New Comment (Ctrl+E: editor, Alt+Enter: submit) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow));
            let inner = input_block.inner(inner_chunks[1]);
            f.render_widget(input_block, inner_chunks[1]);
            f.render_widget(textarea, inner);
        }
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
            Constraint::Min(5),    // Tags
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

    // Description field (uses TextArea)
    if let Some(ref textarea) = app.edit_task_description_textarea {
        let desc_block = Block::default()
            .title(" Description (Ctrl+E: editor) ")
            .borders(Borders::ALL)
            .border_style(field_style(TaskEditField::Description));
        let inner = desc_block.inner(chunks[1]);
        f.render_widget(desc_block, chunks[1]);
        f.render_widget(textarea, inner);
    } else {
        let desc_block = Block::default()
            .title(" Description ")
            .borders(Borders::ALL)
            .border_style(field_style(TaskEditField::Description));
        let desc_text = Paragraph::new("").block(desc_block);
        f.render_widget(desc_text, chunks[1]);
    }

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

    // Render Tags field
    let tag_block = Block::default()
        .title(" Tags (h/l: navigate, Space: toggle) ")
        .borders(Borders::ALL)
        .border_style(field_style(TaskEditField::Tags));

    // Build tag list display
    let tag_lines: Vec<Line> = app.workspace_tags
        .iter()
        .enumerate()
        .map(|(idx, tag)| {
            let is_selected = app.task_edit_selected_tags.contains(&tag.id);
            let is_cursor = app.edit_field == TaskEditField::Tags && idx == app.tag_selector_cursor;
            let checkbox = if is_selected { "[x]" } else { "[ ]" };
            let tag_color = tag.color.as_ref()
                .and_then(|c| parse_hex_color(c))
                .unwrap_or(Color::Gray);

            let style = if is_cursor {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Line::from(vec![
                Span::styled(format!(" {} ", checkbox), style),
                Span::styled(
                    format!(" {} ", tag.name),
                    Style::default().bg(tag_color).fg(Color::Black),
                ),
            ])
        })
        .collect();

    let tags_widget = if tag_lines.is_empty() {
        Paragraph::new("No tags. Press T in kanban to manage tags.").block(tag_block)
    } else {
        Paragraph::new(tag_lines).block(tag_block)
    };
    f.render_widget(tags_widget, chunks[6]);

    // Set cursor position if in insert mode (not for Tags or Description fields - TextArea handles its own cursor)
    if app.vim_mode == VimMode::Insert
        && app.edit_field != TaskEditField::Tags
        && app.edit_field != TaskEditField::Description
    {
        let (cursor_x, cursor_y) = match app.edit_field {
            TaskEditField::Title => (
                chunks[0].x + 1 + app.edit_task_title.len() as u16,
                chunks[0].y + 1,
            ),
            TaskEditField::Description => (chunks[1].x + 1, chunks[1].y + 1), // Not used - TextArea handles cursor
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
            TaskEditField::Tags => (chunks[6].x + 1, chunks[6].y + 1), // Not actually used
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
    } else if app.linking_document_mode {
        "j/k: navigate | Enter: link | Esc: cancel"
    } else if app.unlinking_document_mode {
        "j/k: navigate | Enter: unlink | Esc: cancel"
    } else {
        "e: edit | a: comment | L: link doc | U: unlink doc | q/Esc: back"
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

fn draw_link_document_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, f.area());

    // Clear the background
    f.render_widget(Clear, area);

    // Get available documents (not already linked)
    let linked_ids: std::collections::HashSet<_> = app.task_linked_documents
        .iter()
        .map(|d| d.document_id)
        .collect();

    let available: Vec<_> = app.kb_documents
        .iter()
        .filter(|d| !linked_ids.contains(&d.id))
        .collect();

    let items: Vec<ListItem> = available
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            let style = if i == app.link_document_cursor {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(&doc.title, style),
                Span::styled(format!(" ({})", doc.path), style.fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Link Document (j/k: navigate, Enter: select, Esc: cancel) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(list, area);
}

fn draw_unlink_document_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, f.area());

    // Clear the background
    f.render_widget(Clear, area);

    let items: Vec<ListItem> = app.task_linked_documents
        .iter()
        .enumerate()
        .map(|(i, doc)| {
            let style = if i == app.unlink_document_cursor {
                Style::default().bg(Color::Red).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(&doc.document_title, style),
                Span::styled(format!(" ({})", doc.document_path), style.fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Unlink Document (j/k: navigate, Enter: unlink, Esc: cancel) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        );

    f.render_widget(list, area);
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

fn draw_filter_panel(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 70, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Filter Tasks ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Priority
            Constraint::Length(5), // Tags (scrollable)
            Constraint::Length(3), // Assignee
            Constraint::Length(3), // Due Date
            Constraint::Length(3), // Order By
            Constraint::Length(3), // Actions
            Constraint::Min(0),    // Spacer
            Constraint::Length(2), // Hints
        ])
        .split(inner);

    // Helper for section styling
    let section_style = |section: FilterPanelSection| -> Style {
        if app.filter_panel_section == section {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        }
    };

    // Priority section
    let priorities: &[Option<Priority>] = &[
        None,
        Some(Priority::Highest),
        Some(Priority::High),
        Some(Priority::Medium),
        Some(Priority::Low),
        Some(Priority::Lowest),
    ];
    let mut priority_spans: Vec<Span> = vec![Span::styled(" ", Style::default())];
    for (i, p) in priorities.iter().enumerate() {
        let label = match p {
            None => "None",
            Some(Priority::Highest) => "Highest",
            Some(Priority::High) => "High",
            Some(Priority::Medium) => "Medium",
            Some(Priority::Low) => "Low",
            Some(Priority::Lowest) => "Lowest",
        };
        let is_selected = i == app.filter_priority_cursor;
        let marker = if is_selected { "●" } else { "○" };
        let style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        priority_spans.push(Span::styled(format!("[{} {}] ", marker, label), style));
    }
    let priority_block = Block::default()
        .title(" Priority (h/l) ")
        .borders(Borders::ALL)
        .border_style(section_style(FilterPanelSection::Priority));
    let priority_widget = Paragraph::new(Line::from(priority_spans)).block(priority_block);
    f.render_widget(priority_widget, chunks[0]);

    // Tags section
    let tag_lines: Vec<Line> = if app.workspace_tags.is_empty() {
        vec![Line::from(Span::styled("  No tags available", Style::default().fg(Color::DarkGray)))]
    } else {
        app.workspace_tags
            .iter()
            .enumerate()
            .map(|(i, tag)| {
                let is_selected = app.filter_selected_tags.contains(&tag.id);
                let is_cursor = app.filter_panel_section == FilterPanelSection::Tags && i == app.filter_tag_cursor;
                let checkbox = if is_selected { "[x]" } else { "[ ]" };
                let tag_color = tag.color.as_ref()
                    .and_then(|c| parse_hex_color(c))
                    .unwrap_or(Color::Gray);

                let style = if is_cursor {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                Line::from(vec![
                    Span::styled(format!(" {} ", checkbox), style),
                    Span::styled(
                        format!(" {} ", tag.name),
                        Style::default().bg(tag_color).fg(Color::Black),
                    ),
                ])
            })
            .collect()
    };
    let tag_block = Block::default()
        .title(" Tags (j/k, Space) ")
        .borders(Borders::ALL)
        .border_style(section_style(FilterPanelSection::Tags));
    let tag_widget = Paragraph::new(tag_lines).block(tag_block);
    f.render_widget(tag_widget, chunks[1]);

    // Assignee section
    let assignee_str = if app.filter_assignee_cursor == 0 {
        "None".to_string()
    } else {
        app.workspace_members
            .get(app.filter_assignee_cursor - 1)
            .map(|m| m.display_name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    };
    let assignee_block = Block::default()
        .title(" Assignee (h/l) ")
        .borders(Borders::ALL)
        .border_style(section_style(FilterPanelSection::Assignee));
    let assignee_widget = Paragraph::new(Line::from(vec![
        Span::styled(" < ", Style::default().fg(Color::DarkGray)),
        Span::styled(&assignee_str, Style::default().fg(Color::White)),
        Span::styled(" > ", Style::default().fg(Color::DarkGray)),
    ])).block(assignee_block);
    f.render_widget(assignee_widget, chunks[2]);

    // Due Date section
    let due_mode_str = match app.filter_due_mode {
        DueDateMode::Before => "Before",
        DueDateMode::After => "After",
    };
    let due_date_block = Block::default()
        .title(" Due Date (h/l mode, i edit) ")
        .borders(Borders::ALL)
        .border_style(section_style(FilterPanelSection::DueDate));
    let due_date_widget = Paragraph::new(Line::from(vec![
        Span::styled(format!(" [{}] ", due_mode_str), Style::default().fg(Color::Cyan)),
        Span::styled(
            if app.filter_due_input.is_empty() { "YYYY-MM-DD" } else { &app.filter_due_input },
            Style::default().fg(if app.filter_due_input.is_empty() { Color::DarkGray } else { Color::White }),
        ),
    ])).block(due_date_block);
    f.render_widget(due_date_widget, chunks[3]);

    // Order By section
    let (sort_field, sort_label) = SORT_FIELDS.get(app.filter_order_cursor).unwrap_or(&("position", "Position"));
    let direction = if app.filter_order_desc { "↑" } else { "↓" };
    let order_block = Block::default()
        .title(" Order By (h/l field, Space dir) ")
        .borders(Borders::ALL)
        .border_style(section_style(FilterPanelSection::OrderBy));
    let order_widget = Paragraph::new(Line::from(vec![
        Span::styled(" < ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} {}", sort_label, direction), Style::default().fg(Color::White)),
        Span::styled(" > ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!(" ({})", sort_field), Style::default().fg(Color::DarkGray)),
    ])).block(order_block);
    f.render_widget(order_widget, chunks[4]);

    // Actions section
    let actions_style = section_style(FilterPanelSection::Actions);
    let actions_block = Block::default()
        .title(" Actions ")
        .borders(Borders::ALL)
        .border_style(actions_style);
    let actions_widget = Paragraph::new(Line::from(vec![
        Span::styled(" [Enter: Apply] ", Style::default().fg(Color::Green)),
        Span::styled(" [c: Clear] ", Style::default().fg(Color::Yellow)),
        Span::styled(" [s: Save Preset] ", Style::default().fg(Color::Cyan)),
    ])).block(actions_block);
    f.render_widget(actions_widget, chunks[5]);

    // Hints
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("Tab/j/k", Style::default().fg(Color::Yellow)),
        Span::raw(": section | "),
        Span::styled("h/l", Style::default().fg(Color::Yellow)),
        Span::raw(": value | "),
        Span::styled("Space", Style::default().fg(Color::Yellow)),
        Span::raw(": toggle | "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": cancel"),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(hint, chunks[7]);
}

fn draw_preset_panel(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Filter Presets ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(0),    // Preset list
            Constraint::Length(3), // New preset input (if creating)
            Constraint::Length(2), // Hints
        ])
        .split(inner);

    // Preset list
    let preset_items: Vec<ListItem> = app
        .filter_presets
        .iter()
        .enumerate()
        .map(|(i, preset)| {
            let is_selected = i == app.preset_list_cursor;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            // Build a description of the preset
            let mut desc_parts = Vec::new();
            if preset.filters.priority.is_some() {
                desc_parts.push("priority");
            }
            if preset.filters.assigned_to.is_some() {
                desc_parts.push("assignee");
            }
            if preset.filters.due_before.is_some() || preset.filters.due_after.is_some() {
                desc_parts.push("due date");
            }
            if preset.filters.order_by.is_some() {
                desc_parts.push("sorted");
            }
            let desc = if desc_parts.is_empty() {
                "no filters".to_string()
            } else {
                desc_parts.join(", ")
            };

            ListItem::new(Line::from(vec![
                Span::styled("  ", style),
                Span::styled(&preset.name, style.add_modifier(Modifier::BOLD)),
                Span::styled(format!(" ({})", desc), style.fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list_title = if app.filter_presets.is_empty() {
        " No presets - press 'n' to create ".to_string()
    } else {
        format!(" Presets ({}) ", app.filter_presets.len())
    };

    let list = List::new(preset_items).block(
        Block::default()
            .title(list_title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );
    f.render_widget(list, chunks[0]);

    // New preset input (if creating)
    if app.creating_preset {
        let input_block = Block::default()
            .title(" Preset Name ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let input = Paragraph::new(app.new_preset_name.as_str()).block(input_block);
        f.render_widget(input, chunks[1]);

        // Set cursor position
        f.set_cursor_position((
            chunks[1].x + 1 + app.new_preset_name.len() as u16,
            chunks[1].y + 1,
        ));
    }

    // Hints
    let hint = if app.creating_preset {
        Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": save | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ]))
    } else {
        Paragraph::new(Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(": select | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": load | "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(": new | "),
            Span::styled("d", Style::default().fg(Color::Yellow)),
            Span::raw(": delete | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": close"),
        ]))
    }
    .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);
}

fn draw_knowledge_base(f: &mut Frame, app: &App) {
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

    // Main content: split into tree and document viewer
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Document tree
            Constraint::Percentage(70), // Document content
        ])
        .split(chunks[1]);

    // Draw document tree
    draw_document_tree(f, content_chunks[0], app);

    // Draw document content/editor
    draw_document_content(f, content_chunks[1], app);

    // Status bar
    draw_kb_status_bar(f, chunks[2], app);

    // Draw create document popup if active
    if app.kb_creating {
        draw_kb_create_popup(f, app);
    }

    // Draw delete confirmation popup if active
    if app.kb_confirming_delete {
        draw_kb_delete_confirm_popup(f, app);
    }

    // Draw link task popup if active
    if app.linking_task_mode {
        draw_link_task_popup(f, app);
    }
}

fn draw_document_tree(f: &mut Frame, area: Rect, app: &App) {
    let is_focused = app.kb_focus == KbFocus::Tree;
    let border_color = if is_focused { Color::Cyan } else { Color::DarkGray };

    let block = Block::default()
        .title(" Documents ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.kb_visible_list.is_empty() {
        let empty = Paragraph::new("No documents. Press 'n' to create one.")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);
        f.render_widget(empty, inner);
        return;
    }

    // Build list items from visible tree
    let items: Vec<ListItem> = app
        .kb_visible_list
        .iter()
        .enumerate()
        .map(|(i, (doc, depth))| {
            let is_selected = i == app.kb_selected_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            // Determine expand/collapse indicator (all docs can be expanded)
            let indicator = if app.kb_expanded.contains(&doc.id) {
                "▼ "
            } else {
                "▶ "
            };

            // Build indentation
            let indent = "  ".repeat(*depth);

            // Truncate title if needed
            let available_width = area.width.saturating_sub(4 + (depth * 2) as u16 + 2) as usize;
            let title = if doc.title.len() > available_width {
                format!("{}...", &doc.title[..available_width.saturating_sub(3)])
            } else {
                doc.title.clone()
            };

            ListItem::new(Line::from(vec![
                Span::styled(indent, style),
                Span::styled(indicator, Style::default().fg(Color::Yellow)),
                Span::styled(title, style),
            ]))
        })
        .collect();

    let list = List::new(items);
    f.render_widget(list, inner);
}

fn draw_document_content(f: &mut Frame, area: Rect, app: &App) {
    if app.kb_editing {
        draw_document_editor(f, area, app);
        return;
    }

    let is_focused = app.kb_focus == KbFocus::Content;
    let border_color = if is_focused { Color::Cyan } else { Color::DarkGray };

    match &app.kb_selected_doc {
        Some(doc) => {
            // Calculate linked tasks height (header + items or empty message)
            let linked_tasks_height = if app.kb_linked_tasks.is_empty() {
                3 // Header + "(none)" + padding
            } else {
                (2 + app.kb_linked_tasks.len().min(5)) as u16 // Header + up to 5 tasks
            };

            // Render markdown content to calculate total height
            let content_width = area.width.saturating_sub(4) as usize;
            let content_text = doc.content.as_deref().unwrap_or("");
            let all_content_lines = if content_text.is_empty() {
                vec![Line::from(Span::styled(
                    "(No content)",
                    Style::default().fg(Color::DarkGray),
                ))]
            } else {
                markdown::render_markdown(content_text, content_width)
            };

            let total_lines = all_content_lines.len();
            let visible_height = area.height.saturating_sub(4 + linked_tasks_height) as usize;

            // Clamp scroll offset to valid range
            let max_scroll = total_lines.saturating_sub(visible_height);
            let scroll_offset = app.kb_scroll_offset.min(max_scroll);

            // Build scroll position indicator
            let scroll_indicator = if total_lines <= visible_height {
                String::new()
            } else if scroll_offset == 0 {
                " [Top]".to_string()
            } else if scroll_offset >= max_scroll {
                " [Bot]".to_string()
            } else {
                format!(" [{}%]", scroll_offset * 100 / max_scroll)
            };

            let title = format!(" {} {}", doc.title, scroll_indicator);
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner = block.inner(area);
            f.render_widget(block, area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(3),                       // Content
                    Constraint::Length(linked_tasks_height),  // Linked Tasks
                ])
                .split(inner);

            // Apply scroll offset and render visible lines
            let visible_lines: Vec<Line> = all_content_lines
                .into_iter()
                .skip(scroll_offset)
                .take(visible_height)
                .collect();

            let content = Paragraph::new(visible_lines);
            f.render_widget(content, chunks[0]);

            // Linked Tasks section
            let mut linked_lines = vec![
                Line::from(Span::styled(
                    format!("Linked Tasks ({}):", app.kb_linked_tasks.len()),
                    Style::default().fg(Color::Cyan),
                )),
            ];
            if app.kb_linked_tasks.is_empty() {
                linked_lines.push(Line::from(Span::styled(
                    "  (none)",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                for task in app.kb_linked_tasks.iter().take(5) {
                    linked_lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(&task.task_title, Style::default().fg(Color::Green)),
                    ]));
                }
                if app.kb_linked_tasks.len() > 5 {
                    linked_lines.push(Line::from(Span::styled(
                        format!("  ... and {} more", app.kb_linked_tasks.len() - 5),
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            }
            let linked_tasks = Paragraph::new(linked_lines);
            f.render_widget(linked_tasks, chunks[1]);
        }
        None => {
            let block = Block::default()
                .title(" Content ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            let inner = block.inner(area);
            f.render_widget(block, area);

            let empty = Paragraph::new("Select a document to view its content")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            f.render_widget(empty, inner);
        }
    }
}

fn draw_document_editor(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Edit Document ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title input
            Constraint::Min(0),    // Content input
        ])
        .split(inner);

    // Title input
    let title_block = Block::default()
        .title(" Title ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let title_input = Paragraph::new(app.kb_edit_title.as_str()).block(title_block);
    f.render_widget(title_input, chunks[0]);

    // Content input (uses TextArea)
    if let Some(ref textarea) = app.kb_content_textarea {
        let content_block = Block::default()
            .title(" Content (Ctrl+E: external editor) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));
        let inner = content_block.inner(chunks[1]);
        f.render_widget(content_block, chunks[1]);
        f.render_widget(textarea, inner);
    } else {
        let content_block = Block::default()
            .title(" Content ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray));
        let content_input = Paragraph::new("").block(content_block);
        f.render_widget(content_input, chunks[1]);
    }

    // Don't manually set cursor - TextArea handles it
}

fn draw_kb_status_bar(f: &mut Frame, area: Rect, app: &App) {
    let (mode, mode_color) = if app.kb_editing {
        ("EDIT", Color::Yellow)
    } else if app.kb_creating {
        ("CREATE", Color::Green)
    } else if app.kb_confirming_delete {
        ("DELETE", Color::Red)
    } else if app.linking_task_mode {
        ("LINK", Color::Cyan)
    } else {
        ("NORMAL", Color::Blue)
    };

    let hints = if app.kb_editing {
        "Ctrl+E: editor | Alt+Enter: save | Esc: cancel"
    } else if app.kb_creating {
        "Enter: create | Esc: cancel"
    } else if app.kb_confirming_delete {
        "y: confirm | n/Esc: cancel"
    } else if app.linking_task_mode {
        "j/k: navigate | Enter: link | Esc: cancel"
    } else {
        "j/k: nav | n: new | e: edit | d: del | L: link task | U: unlink | q: close"
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

fn draw_kb_create_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 30, f.area());

    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Document ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3), // Title input
            Constraint::Length(2), // Parent info
            Constraint::Length(2), // Hint
            Constraint::Min(0),    // Spacer
        ])
        .split(inner);

    // Title input
    let title_block = Block::default()
        .title(" Title ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let title_input = Paragraph::new(app.kb_create_title.as_str()).block(title_block);
    f.render_widget(title_input, chunks[0]);

    // Parent info
    let parent_text = match app.kb_create_parent_id {
        Some(pid) => {
            let parent_name = app
                .kb_documents
                .iter()
                .find(|d| d.id == pid)
                .map(|d| d.title.as_str())
                .unwrap_or("Unknown");
            format!("Creating under: {}", parent_name)
        }
        None => "Creating at root level".to_string(),
    };
    let parent_info = Paragraph::new(parent_text)
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(parent_info, chunks[1]);

    // Hint
    let hint = Paragraph::new("Enter: create | Esc: cancel")
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);
    f.render_widget(hint, chunks[2]);

    // Set cursor position
    f.set_cursor_position((
        chunks[0].x + 1 + app.kb_create_title.len() as u16,
        chunks[0].y + 1,
    ));
}

fn draw_kb_delete_confirm_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 20, f.area());

    f.render_widget(Clear, area);

    let doc_title = app
        .kb_visible_list
        .get(app.kb_selected_idx)
        .map(|(doc, _)| doc.title.as_str())
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
        Line::from(Span::raw("Delete document:")),
        Line::from(Span::styled(
            format!("\"{}\"", doc_title),
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

fn draw_link_task_popup(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, f.area());

    // Clear the background
    f.render_widget(Clear, area);

    // Get available tasks (not already linked)
    let linked_ids: std::collections::HashSet<_> = app.kb_linked_tasks
        .iter()
        .map(|t| t.task_id)
        .collect();

    // Get all tasks from columns
    let all_tasks: Vec<_> = app.columns.iter().flat_map(|c| c.tasks.iter()).collect();
    let available: Vec<_> = all_tasks
        .iter()
        .filter(|t| !linked_ids.contains(&t.id))
        .collect();

    let items: Vec<ListItem> = available
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let style = if i == app.link_task_cursor {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(&task.title, style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Link Task (j/k: navigate, Enter: select, Esc: cancel) ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

    f.render_widget(list, area);
}

fn draw_help(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 80, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help - Keyboard Shortcuts ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let help_content = vec![
        Line::from(Span::styled(
            "NAVIGATION",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  h/l     ", Style::default().fg(Color::Green)),
            Span::raw("Move between columns"),
        ]),
        Line::from(vec![
            Span::styled("  j/k     ", Style::default().fg(Color::Green)),
            Span::raw("Move between tasks"),
        ]),
        Line::from(vec![
            Span::styled("  Enter   ", Style::default().fg(Color::Green)),
            Span::raw("Open task details"),
        ]),
        Line::from(vec![
            Span::styled("  Backsp  ", Style::default().fg(Color::Green)),
            Span::raw("Go back"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "TASK ACTIONS",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  n       ", Style::default().fg(Color::Green)),
            Span::raw("New task"),
        ]),
        Line::from(vec![
            Span::styled("  d       ", Style::default().fg(Color::Green)),
            Span::raw("Delete task"),
        ]),
        Line::from(vec![
            Span::styled("  m+h/l   ", Style::default().fg(Color::Green)),
            Span::raw("Move task to column"),
        ]),
        Line::from(vec![
            Span::styled("  e       ", Style::default().fg(Color::Green)),
            Span::raw("Edit (in detail view)"),
        ]),
        Line::from(vec![
            Span::styled("  a       ", Style::default().fg(Color::Green)),
            Span::raw("Add comment (in detail view)"),
        ]),
        Line::from(vec![
            Span::styled("  L       ", Style::default().fg(Color::Green)),
            Span::raw("Link document (in detail view)"),
        ]),
        Line::from(vec![
            Span::styled("  U       ", Style::default().fg(Color::Green)),
            Span::raw("Unlink document (in detail view)"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "SEARCH & FILTER",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  /       ", Style::default().fg(Color::Green)),
            Span::raw("Search"),
        ]),
        Line::from(vec![
            Span::styled("  f       ", Style::default().fg(Color::Green)),
            Span::raw("Toggle filter bar"),
        ]),
        Line::from(vec![
            Span::styled("  F       ", Style::default().fg(Color::Green)),
            Span::raw("Open filter panel"),
        ]),
        Line::from(vec![
            Span::styled("  P       ", Style::default().fg(Color::Green)),
            Span::raw("Filter presets"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "FEATURES",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  Ctrl+P  ", Style::default().fg(Color::Green)),
            Span::raw("Menu (command palette)"),
        ]),
        Line::from(vec![
            Span::styled("  M       ", Style::default().fg(Color::Green)),
            Span::raw("Members"),
        ]),
        Line::from(vec![
            Span::styled("  T       ", Style::default().fg(Color::Green)),
            Span::raw("Tags"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+K  ", Style::default().fg(Color::Green)),
            Span::raw("Knowledge Base"),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl+W  ", Style::default().fg(Color::Green)),
            Span::raw("Workspace switch"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "KNOWLEDGE BASE",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  j/k     ", Style::default().fg(Color::Green)),
            Span::raw("Navigate documents"),
        ]),
        Line::from(vec![
            Span::styled("  h/l     ", Style::default().fg(Color::Green)),
            Span::raw("Collapse/expand"),
        ]),
        Line::from(vec![
            Span::styled("  n       ", Style::default().fg(Color::Green)),
            Span::raw("New document"),
        ]),
        Line::from(vec![
            Span::styled("  e       ", Style::default().fg(Color::Green)),
            Span::raw("Edit document"),
        ]),
        Line::from(vec![
            Span::styled("  d       ", Style::default().fg(Color::Green)),
            Span::raw("Delete document"),
        ]),
        Line::from(vec![
            Span::styled("  L       ", Style::default().fg(Color::Green)),
            Span::raw("Link task to document"),
        ]),
        Line::from(vec![
            Span::styled("  U       ", Style::default().fg(Color::Green)),
            Span::raw("Unlink task from document"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "GENERAL",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("  ?       ", Style::default().fg(Color::Green)),
            Span::raw("This help"),
        ]),
        Line::from(vec![
            Span::styled("  Esc     ", Style::default().fg(Color::Green)),
            Span::raw("Cancel / close"),
        ]),
        Line::from(vec![
            Span::styled("  q       ", Style::default().fg(Color::Green)),
            Span::raw("Quit"),
        ]),
    ];

    // Calculate visible lines based on scroll
    let visible_height = inner.height.saturating_sub(2) as usize;
    let max_scroll = help_content.len().saturating_sub(visible_height);
    let scroll = app.help_scroll.min(max_scroll);

    let visible_content: Vec<Line> = help_content
        .into_iter()
        .skip(scroll)
        .take(visible_height)
        .collect();

    let content = Paragraph::new(visible_content).block(
        Block::default()
            .borders(Borders::NONE)
            .padding(Padding::horizontal(2)),
    );
    f.render_widget(content, inner);

    // Draw scroll indicator and hint at bottom
    let hint_area = Rect {
        x: area.x + 1,
        y: area.y + area.height - 2,
        width: area.width - 2,
        height: 1,
    };
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(": scroll | "),
        Span::styled("q/Esc/?", Style::default().fg(Color::Yellow)),
        Span::raw(": close"),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(hint, hint_area);
}

fn draw_menu(f: &mut Frame, app: &App) {
    let area = centered_rect(35, 40, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Menu ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let menu_items = [
        ("m", "Members"),
        ("k", "Knowledge Base"),
        ("t", "Tags"),
        ("f", "Filters"),
        ("p", "Presets"),
        ("/", "Search"),
        ("w", "Workspaces"),
    ];

    let items: Vec<ListItem> = menu_items
        .iter()
        .enumerate()
        .map(|(i, (key, label))| {
            let is_selected = i == app.menu_selected_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!(" [{}] ", key), Style::default().fg(Color::Yellow)),
                Span::styled(*label, style),
            ]))
        })
        .collect();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    let list = List::new(items).highlight_style(Style::default().bg(Color::DarkGray));
    f.render_widget(list, chunks[0]);

    // Hint at bottom
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(": nav | "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(": select | "),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::raw(": close"),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(hint, chunks[1]);
}

fn draw_workspace_modal(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 60, f.area());
    f.render_widget(Clear, area);

    let title = if app.creating_workspace {
        " Create Workspace "
    } else {
        " Switch Workspace "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Split into list area and hint/input area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(inner);

    if app.creating_workspace {
        // Show input for new workspace name
        let input = Paragraph::new(app.new_workspace_name.as_str())
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(" Workspace Name ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            );
        f.render_widget(input, chunks[0]);

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": create | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": cancel"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hint, chunks[1]);
    } else {
        // Show workspace list
        let current_id = app.current_workspace.as_ref().map(|w| w.id);

        let items: Vec<ListItem> = app
            .workspaces
            .iter()
            .enumerate()
            .map(|(i, ws)| {
                let is_selected = i == app.selected_workspace_idx;
                let is_current = Some(ws.workspace.id) == current_id;

                let style = if is_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default()
                };

                let role_str = match ws.role {
                    todo_shared::WorkspaceRole::Owner => "[Owner]",
                    todo_shared::WorkspaceRole::Admin => "[Admin]",
                    todo_shared::WorkspaceRole::Editor => "[Editor]",
                    todo_shared::WorkspaceRole::Reader => "[Reader]",
                };

                let current_marker = if is_current { " ●" } else { "" };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!(" {}{} ", ws.workspace.name, current_marker),
                        if is_current {
                            style.fg(Color::Green)
                        } else {
                            style
                        },
                    ),
                    Span::styled(
                        role_str,
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(Style::default().bg(Color::DarkGray));
        f.render_widget(list, chunks[0]);

        let hint = Paragraph::new(Line::from(vec![
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(": nav | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": select | "),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::raw(": new | "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(": close"),
        ]))
        .alignment(Alignment::Center);
        f.render_widget(hint, chunks[1]);
    }
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
