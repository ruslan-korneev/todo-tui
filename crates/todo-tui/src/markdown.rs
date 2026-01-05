use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::sync::LazyLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{ThemeSet, Style as SyntectStyle},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// Render markdown content to ratatui Lines
pub fn render_markdown(content: &str, width: usize) -> Vec<Line<'static>> {
    let mut renderer = MarkdownRenderer::new(width);
    renderer.render(content)
}

struct MarkdownRenderer {
    width: usize,
    lines: Vec<Line<'static>>,
    current_spans: Vec<Span<'static>>,
    style_stack: Vec<Style>,
    list_stack: Vec<ListState>,
    in_code_block: bool,
    code_block_lang: Option<String>,
    code_block_content: String,
    in_blockquote: bool,
    in_table: bool,
    table_row: Vec<String>,
    table_alignments: Vec<pulldown_cmark::Alignment>,
    table_rows: Vec<Vec<String>>,
}

#[derive(Clone)]
struct ListState {
    ordered: bool,
    index: usize,
}

impl MarkdownRenderer {
    fn new(width: usize) -> Self {
        Self {
            width,
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default().fg(Color::White)],
            list_stack: Vec::new(),
            in_code_block: false,
            code_block_lang: None,
            code_block_content: String::new(),
            in_blockquote: false,
            in_table: false,
            table_row: Vec::new(),
            table_alignments: Vec::new(),
            table_rows: Vec::new(),
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, style: Style) {
        let current = self.current_style();
        self.style_stack.push(current.patch(style));
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let prefix = if self.in_blockquote {
                vec![Span::styled("│ ", Style::default().fg(Color::DarkGray))]
            } else {
                vec![]
            };
            let mut spans = prefix;
            spans.append(&mut self.current_spans);
            self.lines.push(Line::from(spans));
            self.current_spans = Vec::new();
        }
    }

    fn add_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_content.push_str(text);
            return;
        }

        if self.in_table {
            if let Some(last) = self.table_row.last_mut() {
                last.push_str(text);
            }
            return;
        }

        let style = self.current_style();

        // Handle word wrapping
        let available_width = if self.in_blockquote {
            self.width.saturating_sub(2)
        } else {
            self.width
        };

        for part in text.split('\n') {
            if !self.current_spans.is_empty() || !part.is_empty() {
                // Calculate current line length
                let current_len: usize = self.current_spans.iter().map(|s| s.content.len()).sum();

                if current_len + part.len() > available_width && current_len > 0 {
                    self.flush_line();
                }

                // Check if text starts/ends with whitespace (need to preserve spaces between spans)
                let starts_with_space = part.starts_with(char::is_whitespace);
                let ends_with_space = part.ends_with(char::is_whitespace);

                // Word wrap long text
                let words: Vec<&str> = part.split_whitespace().collect();
                let mut line_text = String::new();

                // If there are existing spans and text starts with space, add leading space
                if starts_with_space && !self.current_spans.is_empty() && !words.is_empty() {
                    line_text.push(' ');
                }

                for word in words {
                    let current_line_len: usize = self.current_spans.iter().map(|s| s.content.len()).sum();
                    let test_len = current_line_len + line_text.len() + word.len() + 1;

                    if test_len > available_width && !line_text.is_empty() {
                        self.current_spans.push(Span::styled(line_text.clone(), style));
                        self.flush_line();
                        line_text.clear();
                    }

                    if !line_text.is_empty() && !line_text.ends_with(' ') {
                        line_text.push(' ');
                    }
                    line_text.push_str(word);
                }

                // Preserve trailing space for next span
                if ends_with_space && !line_text.is_empty() {
                    line_text.push(' ');
                }

                if !line_text.is_empty() {
                    self.current_spans.push(Span::styled(line_text, style));
                }
            }
        }
    }

    fn render_code_block(&mut self) {
        let lang = self.code_block_lang.take();
        let content = std::mem::take(&mut self.code_block_content);

        let syntax = lang
            .as_ref()
            .and_then(|l| SYNTAX_SET.find_syntax_by_token(l))
            .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

        let theme = &THEME_SET.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let bg_style = Style::default().bg(Color::Rgb(43, 48, 59));

        for line in LinesWithEndings::from(&content) {
            let mut spans = Vec::new();

            if let Ok(highlighted) = highlighter.highlight_line(line, &SYNTAX_SET) {
                for (style, text) in highlighted {
                    let color = syntect_to_ratatui_color(style);
                    let text = text.trim_end_matches('\n').to_string();
                    if !text.is_empty() {
                        spans.push(Span::styled(text, bg_style.fg(color)));
                    }
                }
            } else {
                spans.push(Span::styled(
                    line.trim_end_matches('\n').to_string(),
                    bg_style.fg(Color::White),
                ));
            }

            // Pad line to width for consistent background
            let line_len: usize = spans.iter().map(|s| s.content.len()).sum();
            if line_len < self.width {
                spans.push(Span::styled(
                    " ".repeat(self.width - line_len),
                    bg_style,
                ));
            }

            self.lines.push(Line::from(spans));
        }

        self.lines.push(Line::from(""));
    }

    fn render_table(&mut self) {
        let rows = std::mem::take(&mut self.table_rows);
        let _alignments = std::mem::take(&mut self.table_alignments);

        if rows.is_empty() {
            return;
        }

        // Calculate column widths
        let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(0);
        let mut col_widths: Vec<usize> = vec![0; col_count];

        for row in &rows {
            for (i, cell) in row.iter().enumerate() {
                if i < col_widths.len() {
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
        }

        let border_style = Style::default().fg(Color::DarkGray);
        let header_style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        let cell_style = Style::default().fg(Color::White);

        // Top border
        let top_border = format!(
            "┌{}┐",
            col_widths
                .iter()
                .map(|w| "─".repeat(w + 2))
                .collect::<Vec<_>>()
                .join("┬")
        );
        self.lines.push(Line::from(Span::styled(top_border, border_style)));

        for (row_idx, row) in rows.iter().enumerate() {
            let mut spans = vec![Span::styled("│", border_style)];

            for (col_idx, cell) in row.iter().enumerate() {
                let width = col_widths.get(col_idx).copied().unwrap_or(0);
                let padded = format!(" {:width$} ", cell, width = width);
                let style = if row_idx == 0 { header_style } else { cell_style };
                spans.push(Span::styled(padded, style));
                spans.push(Span::styled("│", border_style));
            }

            self.lines.push(Line::from(spans));

            // Header separator
            if row_idx == 0 && rows.len() > 1 {
                let sep = format!(
                    "├{}┤",
                    col_widths
                        .iter()
                        .map(|w| "─".repeat(w + 2))
                        .collect::<Vec<_>>()
                        .join("┼")
                );
                self.lines.push(Line::from(Span::styled(sep, border_style)));
            }
        }

        // Bottom border
        let bottom_border = format!(
            "└{}┘",
            col_widths
                .iter()
                .map(|w| "─".repeat(w + 2))
                .collect::<Vec<_>>()
                .join("┴")
        );
        self.lines.push(Line::from(Span::styled(bottom_border, border_style)));
        self.lines.push(Line::from(""));
    }

    fn render(&mut self, content: &str) -> Vec<Line<'static>> {
        let options = Options::ENABLE_TABLES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS;

        let parser = Parser::new_ext(content, options);

        for event in parser {
            match event {
                Event::Start(tag) => self.handle_start_tag(tag),
                Event::End(tag) => self.handle_end_tag(tag),
                Event::Text(text) => self.add_text(&text),
                Event::Code(code) => {
                    let style = Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::Rgb(50, 50, 50));
                    self.current_spans.push(Span::styled(format!("`{}`", code), style));
                }
                Event::SoftBreak => {
                    self.current_spans.push(Span::raw(" "));
                }
                Event::HardBreak => {
                    self.flush_line();
                }
                Event::Rule => {
                    self.flush_line();
                    let rule = "─".repeat(self.width.min(60));
                    self.lines.push(Line::from(Span::styled(
                        rule,
                        Style::default().fg(Color::DarkGray),
                    )));
                    self.lines.push(Line::from(""));
                }
                Event::TaskListMarker(checked) => {
                    let marker = if checked { "☑ " } else { "☐ " };
                    self.current_spans.push(Span::styled(
                        marker.to_string(),
                        Style::default().fg(Color::Cyan),
                    ));
                }
                _ => {}
            }
        }

        self.flush_line();
        std::mem::take(&mut self.lines)
    }

    fn handle_start_tag(&mut self, tag: Tag) {
        match tag {
            Tag::Heading { level, .. } => {
                self.flush_line();
                let (style, prefix) = match level {
                    HeadingLevel::H1 => (
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                        "# ",
                    ),
                    HeadingLevel::H2 => (
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                        "## ",
                    ),
                    _ => (
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                        "",
                    ),
                };
                self.push_style(style);
                if !prefix.is_empty() {
                    self.current_spans.push(Span::styled(prefix.to_string(), style));
                }
            }
            Tag::Paragraph => {
                self.flush_line();
            }
            Tag::BlockQuote => {
                self.flush_line();
                self.in_blockquote = true;
                self.push_style(Style::default().fg(Color::Gray));
            }
            Tag::CodeBlock(kind) => {
                self.flush_line();
                self.in_code_block = true;
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.to_string();
                        if lang.is_empty() { None } else { Some(lang) }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(start) => {
                self.flush_line();
                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    index: start.unwrap_or(1) as usize,
                });
            }
            Tag::Item => {
                self.flush_line();
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));

                if let Some(list) = self.list_stack.last_mut() {
                    let marker = if list.ordered {
                        let m = format!("{}. ", list.index);
                        list.index += 1;
                        m
                    } else {
                        "• ".to_string()
                    };
                    self.current_spans.push(Span::styled(
                        format!("{}{}", indent, marker),
                        Style::default().fg(Color::Cyan),
                    ));
                }
            }
            Tag::Emphasis => {
                self.push_style(Style::default().add_modifier(Modifier::ITALIC));
            }
            Tag::Strong => {
                self.push_style(Style::default().add_modifier(Modifier::BOLD));
            }
            Tag::Strikethrough => {
                self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT));
            }
            Tag::Link { .. } => {
                self.push_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Tag::Table(alignments) => {
                self.flush_line();
                self.in_table = true;
                self.table_alignments = alignments;
                self.table_rows.clear();
            }
            Tag::TableHead | Tag::TableRow => {
                self.table_row.clear();
            }
            Tag::TableCell => {
                self.table_row.push(String::new());
            }
            _ => {}
        }
    }

    fn handle_end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Heading(_) => {
                self.flush_line();
                self.pop_style();
                self.lines.push(Line::from(""));
            }
            TagEnd::Paragraph => {
                self.flush_line();
                self.lines.push(Line::from(""));
            }
            TagEnd::BlockQuote => {
                self.flush_line();
                self.in_blockquote = false;
                self.pop_style();
                self.lines.push(Line::from(""));
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.render_code_block();
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.lines.push(Line::from(""));
                }
            }
            TagEnd::Item => {
                self.flush_line();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.pop_style();
            }
            TagEnd::Table => {
                self.in_table = false;
                self.render_table();
            }
            TagEnd::TableHead | TagEnd::TableRow => {
                let row = std::mem::take(&mut self.table_row);
                self.table_rows.push(row);
            }
            TagEnd::TableCell => {}
            _ => {}
        }
    }
}

fn syntect_to_ratatui_color(style: SyntectStyle) -> Color {
    Color::Rgb(
        style.foreground.r,
        style.foreground.g,
        style.foreground.b,
    )
}
