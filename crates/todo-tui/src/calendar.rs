//! Mini calendar widget for the Home view
//! Displays a month calendar with highlighted days that have tasks due

use chrono::{Datelike, NaiveDate};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::collections::HashMap;

/// Get the number of days in a month
fn days_in_month(year: i32, month: u32) -> u32 {
    // Move to next month, then back one day
    if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    }
    .and_then(|d| d.pred_opt())
    .map(|d| d.day())
    .unwrap_or(30)
}

/// Get month name
pub fn month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

/// Render calendar as lines for ratatui
/// Returns a Vec of styled Lines representing the calendar grid
pub fn render_calendar(
    year: i32,
    month: u32,
    tasks: &HashMap<NaiveDate, usize>,
    today: NaiveDate,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Weekday header
    let header_style = Style::default().fg(Color::DarkGray);
    lines.push(Line::from(vec![
        Span::styled("Su ", header_style),
        Span::styled("Mo ", header_style),
        Span::styled("Tu ", header_style),
        Span::styled("We ", header_style),
        Span::styled("Th ", header_style),
        Span::styled("Fr ", header_style),
        Span::styled("Sa", header_style),
    ]));

    // Get first day of month
    let first_day = match NaiveDate::from_ymd_opt(year, month, 1) {
        Some(d) => d,
        None => return lines,
    };

    // 0 = Sunday, 1 = Monday, ... 6 = Saturday
    let start_weekday = first_day.weekday().num_days_from_sunday() as usize;
    let num_days = days_in_month(year, month);

    let mut current_day = 1u32;

    // Build up to 6 week rows
    for week in 0..6 {
        let mut spans = Vec::new();

        for weekday in 0..7 {
            let cell_idx = week * 7 + weekday;

            if cell_idx < start_weekday || current_day > num_days {
                // Empty cell
                spans.push(Span::raw(if weekday == 6 { "  " } else { "   " }));
            } else {
                let date = NaiveDate::from_ymd_opt(year, month, current_day).unwrap();
                let task_count = tasks.get(&date).copied().unwrap_or(0);
                let is_today = date == today;

                // Determine style
                let style = if is_today {
                    Style::default()
                        .bg(Color::Blue)
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else if task_count > 0 {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                // Format day number (2 chars + space, except last column)
                let text = if weekday == 6 {
                    format!("{:2}", current_day)
                } else {
                    format!("{:2} ", current_day)
                };
                spans.push(Span::styled(text, style));

                current_day += 1;
            }
        }

        lines.push(Line::from(spans));

        // Stop if we've rendered all days
        if current_day > num_days {
            break;
        }
    }

    lines
}
