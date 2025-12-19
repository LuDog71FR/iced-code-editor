//! Clipboard operations (copy, paste, delete selection).

use iced::Task;

use super::{CanvasEditor, CanvasEditorMessage};

impl CanvasEditor {
    /// Copies selected text to clipboard.
    pub(crate) fn copy_selection(&self) -> Task<CanvasEditorMessage> {
        if let Some(text) = self.get_selected_text() {
            iced::clipboard::write(text)
        } else {
            Task::none()
        }
    }

    /// Deletes the selected text.
    pub(crate) fn delete_selection(&mut self) {
        if let Some((start, end)) = self.get_selection_range() {
            if start == end {
                return; // No selection
            }

            // Delete character by character from end to start
            // This is simpler than implementing range deletion in TextBuffer
            for line_idx in (start.0..=end.0).rev() {
                if line_idx == start.0 && line_idx == end.0 {
                    // Single line selection
                    for _ in start.1..end.1 {
                        self.buffer.delete_forward(start.0, start.1);
                    }
                } else if line_idx == start.0 {
                    // First line - delete from start to end of line
                    let line_len = self.buffer.line_len(line_idx);
                    for _ in start.1..line_len {
                        self.buffer.delete_forward(line_idx, start.1);
                    }
                    // Delete newline
                    if line_idx < self.buffer.line_count() {
                        self.buffer.delete_forward(line_idx, start.1);
                    }
                } else if line_idx == end.0 {
                    // Last line - delete from start to end position
                    for _ in 0..end.1 {
                        self.buffer.delete_forward(start.0, start.1);
                    }
                } else {
                    // Middle line - delete entire line
                    let line_len = self.buffer.line_len(start.0);
                    for _ in 0..line_len {
                        self.buffer.delete_forward(start.0, start.1);
                    }
                    // Delete newline
                    if start.0 < self.buffer.line_count() {
                        self.buffer.delete_forward(start.0, start.1);
                    }
                }
            }

            // Move cursor to selection start
            self.cursor = start;
            self.clear_selection();
        }
    }

    /// Pastes text from clipboard at cursor position.
    pub(crate) fn paste_text(&mut self, text: &str) {
        // If there's a selection, delete it first
        if self.selection_start.is_some() && self.selection_end.is_some() {
            self.delete_selection();
        }

        // Insert text character by character
        for ch in text.chars() {
            if ch == '\n' {
                let (line, col) = self.cursor;
                self.buffer.insert_newline(line, col);
                self.cursor = (line + 1, 0);
            } else {
                let (line, col) = self.cursor;
                self.buffer.insert_char(line, col, ch);
                self.cursor.1 += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_selection_single_line() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        editor.delete_selection();
        assert_eq!(editor.buffer.line(0), " world");
    }
}
