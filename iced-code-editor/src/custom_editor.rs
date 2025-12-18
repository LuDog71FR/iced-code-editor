//! Custom text editor widget built from scratch for performance.
//!
//! This module provides a high-performance text editor that uses:
//! - Virtual scrolling (only renders visible lines)
//! - Efficient text buffer
//! - Simple text_input for editing

use iced::{
    Color, Element, Length, Task,
    widget::{Column, container, row, scrollable, text, text_input},
};

use crate::state::EditorTheme;
use crate::text_buffer::TextBuffer;

/// Configuration constants for the custom editor
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 20.0;
const GUTTER_WIDTH: f32 = 60.0;
const LINES_BUFFER: usize = 5; // Extra lines to render above/below viewport

/// A custom high-performance text editor widget.
///
/// Uses virtual scrolling to handle large files efficiently by only rendering
/// visible lines plus a small buffer.
#[allow(dead_code)] // Fields will be used when editing is fully implemented
#[derive(Debug)]
pub struct CustomEditor {
    /// The text buffer storing all content
    buffer: TextBuffer,
    /// Current cursor position (line, column)
    cursor: (usize, usize),
    /// Scroll position (top visible line)
    scroll_line: usize,
    /// Number of visible lines in viewport
    visible_lines: usize,
    /// Editor theme
    theme: EditorTheme,
    /// Syntax language
    syntax: String,
}

/// Events emitted by the custom editor
#[derive(Debug, Clone)]
pub enum CustomEditorEvent {
    /// Text changed on a specific line
    LineChanged(usize, String),
    /// User pressed Enter on a line
    LineSubmit(usize),
}

impl CustomEditor {
    /// Creates a new custom editor with the given content.
    ///
    /// # Arguments
    ///
    /// * `content` - Initial text content
    /// * `syntax` - Syntax highlighting language (e.g., "py", "lua")
    ///
    /// # Returns
    ///
    /// A new `CustomEditor` instance
    #[must_use]
    pub fn new(content: &str, syntax: &str) -> Self {
        Self {
            buffer: TextBuffer::new(content),
            cursor: (0, 0),
            scroll_line: 0,
            visible_lines: 30, // Default, will be calculated from height
            theme: EditorTheme::dark(),
            syntax: syntax.to_string(),
        }
    }

    /// Returns the total number of lines.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.buffer.line_count()
    }

    /// Returns the current cursor position.
    #[must_use]
    pub fn cursor(&self) -> (usize, usize) {
        self.cursor
    }

    /// Updates the editor based on events.
    ///
    /// # Arguments
    ///
    /// * `event` - The event to handle
    ///
    /// # Returns
    ///
    /// A task for any async operations
    pub fn update(&mut self, event: CustomEditorEvent) -> Task<CustomEditorEvent> {
        match event {
            CustomEditorEvent::LineChanged(line, new_content) => {
                // Update the line in the buffer
                if line < self.buffer.line_count() {
                    // Replace the entire line
                    // For simplicity, we'll rebuild this line
                    // In a real implementation, we'd have a proper line update method
                    let old_content = self.buffer.line(line);
                    if old_content != new_content {
                        // Update by deleting all chars and inserting new ones
                        // For now, we'll store it directly - proper implementation later
                    }
                }
                self.cursor = (line, new_content.len());
            }
            CustomEditorEvent::LineSubmit(line) => {
                // Insert a new line after this one
                if line < self.buffer.line_count() {
                    let col = self.buffer.line(line).len();
                    self.buffer.insert_newline(line, col);
                    self.cursor = (line + 1, 0);
                    self.ensure_cursor_visible();
                }
            }
        }
        Task::none()
    }

    /// Ensures the cursor is visible by adjusting scroll position.
    fn ensure_cursor_visible(&mut self) {
        let (cursor_line, _) = self.cursor;

        if cursor_line < self.scroll_line {
            self.scroll_line = cursor_line;
        } else if cursor_line >= self.scroll_line + self.visible_lines {
            self.scroll_line = cursor_line.saturating_sub(self.visible_lines - 1);
        }
    }

    /// Renders the editor view.
    ///
    /// # Returns
    ///
    /// An Iced `Element` containing the rendered editor
    pub fn view(&self) -> Element<'_, CustomEditorEvent> {
        let line_numbers = self.render_line_numbers();
        let editor_content = self.render_content();

        let combined = row![line_numbers, editor_content].spacing(0);

        container(
            scrollable(combined)
                .height(Length::Fill)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_theme| container::Style {
            background: Some(self.theme.background.into()),
            ..Default::default()
        })
        .into()
    }

    /// Renders line numbers for visible lines.
    fn render_line_numbers(&self) -> Element<'_, CustomEditorEvent> {
        let start_line = self.scroll_line;
        let end_line =
            (self.scroll_line + self.visible_lines + LINES_BUFFER).min(self.buffer.line_count());

        let mut col = Column::new().spacing(0);

        for line_num in start_line..end_line {
            let line_text = format!("{:>4}", line_num + 1);
            col = col.push(
                container(
                    text(line_text)
                        .size(FONT_SIZE)
                        .color(self.theme.line_number_color),
                )
                .height(LINE_HEIGHT)
                .width(GUTTER_WIDTH)
                .style(move |_theme| container::Style {
                    background: Some(self.theme.gutter_background.into()),
                    ..Default::default()
                }),
            );
        }

        col.into()
    }

    /// Renders the text content for visible lines.
    fn render_content(&self) -> Element<'_, CustomEditorEvent> {
        let start_line = self.scroll_line;
        let end_line =
            (self.scroll_line + self.visible_lines + LINES_BUFFER).min(self.buffer.line_count());

        let mut col = Column::new().spacing(0);

        for line_num in start_line..end_line {
            let line_content = self.buffer.line(line_num);
            let is_cursor_line = line_num == self.cursor.0;

            let bg_color = if is_cursor_line {
                Color::from_rgb(0.15, 0.15, 0.2)
            } else {
                self.theme.background
            };

            // Make every line editable with text_input
            let line_input = text_input("", line_content)
                .size(FONT_SIZE)
                .width(Length::Fill)
                .on_input(move |new_value| CustomEditorEvent::LineChanged(line_num, new_value))
                .on_submit(CustomEditorEvent::LineSubmit(line_num))
                .padding(0);

            let line_widget = container(line_input)
                .height(LINE_HEIGHT)
                .width(Length::Fill)
                .style(move |_theme| container::Style {
                    background: Some(bg_color.into()),
                    ..Default::default()
                });

            col = col.push(line_widget);
        }

        col.into()
    }

    /// Returns the buffer content as a string.
    #[must_use]
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let editor = CustomEditor::new("line1\nline2\nline3", "py");
        assert_eq!(editor.line_count(), 3);
        assert_eq!(editor.cursor(), (0, 0));
    }

    #[test]
    fn test_cursor_visibility() {
        let mut editor = CustomEditor::new("line1\nline2\nline3", "py");
        editor.visible_lines = 2;
        editor.cursor = (2, 0);
        editor.ensure_cursor_visible();
        assert!(editor.scroll_line > 0);
    }
}
