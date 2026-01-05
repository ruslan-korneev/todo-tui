use anyhow::Result;
use crossterm::{
    cursor::MoveTo,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::process::Command;
use tempfile::NamedTempFile;
use tui_textarea::TextArea;

/// Editor context determines the editing behavior and styling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorContext {
    TaskDescription,
    DocumentContent,
    Comment,
    NewTaskDescription,
}

/// Create a configured TextArea for the given context
pub fn create_textarea<'a>(content: &str, _context: EditorContext) -> TextArea<'a> {
    let mut textarea = TextArea::default();

    // Set initial content line by line
    if !content.is_empty() {
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            textarea.insert_str(line);
            if i < lines.len() - 1 {
                textarea.insert_newline();
            }
        }
        // If content ends with newline, add it
        if content.ends_with('\n') {
            textarea.insert_newline();
        }
    }

    // Move cursor to the end
    textarea.move_cursor(tui_textarea::CursorMove::Bottom);
    textarea.move_cursor(tui_textarea::CursorMove::End);

    // Configure undo history
    textarea.set_max_histories(100);

    textarea
}

/// Extract content from TextArea as a single String
pub fn textarea_content(textarea: &TextArea) -> String {
    textarea.lines().join("\n")
}

/// Launch external editor with current content, return edited content
pub fn launch_external_editor(content: &str, file_extension: &str) -> Result<String> {
    // Get editor from environment, fallback to vim
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string());

    // Create temp file with content
    let mut temp_file = NamedTempFile::with_suffix(file_extension)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.flush()?;
    let temp_path = temp_file.path().to_path_buf();

    // Leave TUI mode
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;

    // Run editor
    let status = Command::new(&editor).arg(&temp_path).status();

    // Restore TUI mode (always, even on error)
    let restore_result = (|| -> Result<()> {
        execute!(io::stdout(), EnterAlternateScreen)?;
        // Clear terminal and reset cursor to fix display after editor
        execute!(io::stdout(), Clear(ClearType::All), MoveTo(0, 0))?;
        enable_raw_mode()?;
        Ok(())
    })();

    // Handle restore errors
    if let Err(e) = restore_result {
        anyhow::bail!("Failed to restore terminal: {}", e);
    }

    // Check editor result
    match status {
        Ok(exit_status) if exit_status.success() => {
            // Read back the edited content
            let edited = std::fs::read_to_string(&temp_path)?;
            Ok(edited)
        }
        Ok(exit_status) => {
            anyhow::bail!("Editor exited with status: {}", exit_status)
        }
        Err(e) => {
            anyhow::bail!("Failed to launch editor '{}': {}", editor, e)
        }
    }
}
