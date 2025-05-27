use super::syntax_highlighter::RustSyntaxHighlighter;
use super::text_editor::TextEditor;
use super::Widget;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub struct SyntaxEditorWidget<'a> { // Renamed struct
    pub editor: &'a TextEditor,
    pub show_line_numbers: bool,
    pub language: CodeLanguage,
    pub highlighter: RustSyntaxHighlighter,
}

#[derive(Debug, Clone)]
pub enum CodeLanguage {
    Rust,
    Python,
    JavaScript,
    Other,
}

impl<'a> SyntaxEditorWidget<'a> { // Renamed struct
    pub fn new(editor: &'a TextEditor) -> Self {
        Self {
            editor,
            show_line_numbers: true,
            language: CodeLanguage::Rust,
            highlighter: RustSyntaxHighlighter::default(),
        }
    }

    pub fn with_language(mut self, language: CodeLanguage) -> Self {
        self.language = language;
        self
    }

    fn get_syntax_highlighted_lines(&self) -> Vec<Line> {
        let lines = self.editor.lines();
        let (cursor_line, cursor_col) = self.editor.cursor_position();

        lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let mut spans = Vec::new();

                // Add line number
                if self.show_line_numbers {
                    let line_num_style = if i == cursor_line {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    };
                    spans.push(Span::styled(format!("{:3} │ ", i + 1), line_num_style));
                }

                // Add syntax highlighted content
                let highlighted_line = self.highlighter.highlight_line(line);
                spans.extend(highlighted_line.spans);

                // Add cursor indicator if this is the cursor line
                if i == cursor_line {
                    // Insert cursor at the correct position
                    self.insert_cursor_in_spans(&mut spans, cursor_col);
                }

                Line::from(spans)
            })
            .collect()
    }

    fn insert_cursor_in_spans(&self, spans: &mut Vec<Span>, cursor_col: usize) {
        // Skip line number span if present
        let start_index = if self.show_line_numbers { 1 } else { 0 };
        let mut current_col = 0;
        let mut span_index = start_index;

        // If cursor is at position 0 (start of line content), insert before first content span
        if cursor_col == 0 && spans.len() > start_index {
            spans.insert(
                start_index,
                Span::styled("█", Style::default().fg(Color::White).bg(Color::Blue)),
            );
            return;
        }

        while span_index < spans.len() {
            let span = &spans[span_index];
            let span_len = span.content.chars().count();

            if current_col + span_len >= cursor_col {
                // Cursor is within this span or at its end
                let offset_in_span = cursor_col - current_col;
                let span_content = span.content.as_ref();
                let chars: Vec<char> = span_content.chars().collect();

                if offset_in_span < chars.len() {
                    // Split the span and insert cursor
                    let before: String = chars[..offset_in_span].iter().collect();
                    let after: String = chars[offset_in_span..].iter().collect();

                    let original_style = span.style;
                    spans.remove(span_index);

                    if !before.is_empty() {
                        spans.insert(span_index, Span::styled(before, original_style));
                        span_index += 1;
                    }

                    // Insert cursor
                    spans.insert(
                        span_index,
                        Span::styled("█", Style::default().fg(Color::White).bg(Color::Blue)),
                    );
                    span_index += 1;

                    if !after.is_empty() {
                        spans.insert(span_index, Span::styled(after, original_style));
                    }
                } else {
                    // Cursor is at the end of this span
                    spans.insert(
                        span_index + 1,
                        Span::styled("█", Style::default().fg(Color::White).bg(Color::Blue)),
                    );
                }
                return;
            }

            current_col += span_len;
            span_index += 1;
        }

        // If we didn't find the position, add cursor at the end
        spans.push(Span::styled(
            "█",
            Style::default().fg(Color::White).bg(Color::Blue),
        ));
    }

    fn get_editor_info(&self) -> String {
        let total_lines = self.editor.line_count();
        let total_chars = self.editor.char_count();
        let (cursor_line, cursor_col) = self.editor.cursor_position();

        format!(
            "Lines: {} | Chars: {} | Pos: {}:{} | Lang: {:?}",
            total_lines,
            total_chars,
            cursor_line + 1,
            cursor_col + 1,
            self.language
        )
    }
}

impl<'a> Widget for SyntaxEditorWidget<'a> { // Renamed struct
    fn render(&self, f: &mut Frame, area: Rect) {
        let lines = self.get_syntax_highlighted_lines();

        let block = Block::default()
            .title(format!("🦀 Code Editor - {}", self.get_editor_info()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((self.editor.scroll_offset() as u16, 0))
            .block(block);

        f.render_widget(paragraph, area);
    }

    fn title(&self) -> Option<&str> {
        Some("Code Editor")
    }

    fn border_style(&self) -> Style {
        Style::default().fg(Color::Green)
    }
}
