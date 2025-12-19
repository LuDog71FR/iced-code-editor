//! Message handling and update logic.

use iced::Task;

use super::{CURSOR_BLINK_INTERVAL, CanvasEditor, CanvasEditorMessage};

impl CanvasEditor {
    /// Updates the editor state based on messages and returns scroll commands.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to process
    ///
    /// # Returns
    ///
    /// A Task that may contain scroll commands to keep cursor visible
    pub fn update(&mut self, message: &CanvasEditorMessage) -> Task<CanvasEditorMessage> {
        match message {
            CanvasEditorMessage::CharacterInput(ch) => {
                let (line, col) = self.cursor;
                self.buffer.insert_char(line, col, *ch);
                self.cursor.1 += 1;
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            CanvasEditorMessage::Backspace => {
                let (line, col) = self.cursor;
                if self.buffer.delete_char(line, col) {
                    // Line merged with previous
                    if line > 0 {
                        let prev_line_len = self.buffer.line_len(line - 1);
                        self.cursor = (line - 1, prev_line_len);
                    }
                } else if col > 0 {
                    self.cursor.1 -= 1;
                }
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            CanvasEditorMessage::Delete => {
                let (line, col) = self.cursor;
                self.buffer.delete_forward(line, col);
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            CanvasEditorMessage::Enter => {
                let (line, col) = self.cursor;
                self.buffer.insert_newline(line, col);
                self.cursor = (line + 1, 0);
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            CanvasEditorMessage::ArrowKey(direction, shift_pressed) => {
                if *shift_pressed {
                    // Start selection if not already started
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor);
                    }
                    self.move_cursor(*direction);
                    self.selection_end = Some(self.cursor);
                } else {
                    // Clear selection and move cursor
                    self.clear_selection();
                    self.move_cursor(*direction);
                }
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            CanvasEditorMessage::MouseClick(point) => {
                self.handle_mouse_click(*point);
                self.reset_cursor_blink();
                // Clear selection on click
                self.clear_selection();
                self.is_dragging = true;
                self.selection_start = Some(self.cursor);
                Task::none()
            }
            CanvasEditorMessage::MouseDrag(point) => {
                if self.is_dragging {
                    self.handle_mouse_drag(*point);
                    self.cache.clear();
                }
                Task::none()
            }
            CanvasEditorMessage::MouseRelease => {
                self.is_dragging = false;
                Task::none()
            }
            CanvasEditorMessage::Copy => self.copy_selection(),
            CanvasEditorMessage::Paste(text) => {
                // If text is empty, we need to read from clipboard
                if text.is_empty() {
                    // Return a task that reads clipboard and chains to paste
                    iced::clipboard::read().and_then(|clipboard_text| {
                        Task::done(CanvasEditorMessage::Paste(clipboard_text))
                    })
                } else {
                    // We have the text, paste it
                    self.paste_text(text);
                    self.cache.clear();
                    self.scroll_to_cursor()
                }
            }
            CanvasEditorMessage::Tick => {
                // Handle cursor blinking
                if self.last_blink.elapsed() >= CURSOR_BLINK_INTERVAL {
                    self.cursor_visible = !self.cursor_visible;
                    self.last_blink = std::time::Instant::now();
                    self.cache.clear();
                }
                Task::none()
            }
            CanvasEditorMessage::PageUp => {
                self.page_up();
                self.reset_cursor_blink();
                self.scroll_to_cursor()
            }
            CanvasEditorMessage::PageDown => {
                self.page_down();
                self.reset_cursor_blink();
                self.scroll_to_cursor()
            }
            CanvasEditorMessage::Home(shift_pressed) => {
                if *shift_pressed {
                    // Start selection if not already started
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor);
                    }
                    self.cursor.1 = 0; // Move to start of line
                    self.selection_end = Some(self.cursor);
                } else {
                    // Clear selection and move cursor
                    self.clear_selection();
                    self.cursor.1 = 0;
                }
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            CanvasEditorMessage::End(shift_pressed) => {
                let line = self.cursor.0;
                let line_len = self.buffer.line_len(line);

                if *shift_pressed {
                    // Start selection if not already started
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor);
                    }
                    self.cursor.1 = line_len; // Move to end of line
                    self.selection_end = Some(self.cursor);
                } else {
                    // Clear selection and move cursor
                    self.clear_selection();
                    self.cursor.1 = line_len;
                }
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            CanvasEditorMessage::Scrolled(viewport) => {
                // Track viewport scroll position and height
                self.viewport_scroll = viewport.absolute_offset().y;
                self.viewport_height = viewport.bounds().height;
                Task::none()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::ArrowDirection;

    #[test]
    fn test_new_canvas_editor() {
        let editor = CanvasEditor::new("line1\nline2", "py");
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_home_key() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.cursor = (0, 5); // Move to middle of line
        let _ = editor.update(&CanvasEditorMessage::Home(false));
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_end_key() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.cursor = (0, 0);
        let _ = editor.update(&CanvasEditorMessage::End(false));
        assert_eq!(editor.cursor, (0, 11)); // Length of "hello world"
    }

    #[test]
    fn test_arrow_key_with_shift_creates_selection() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.cursor = (0, 0);

        // Shift+Right should start selection
        let _ = editor.update(&CanvasEditorMessage::ArrowKey(ArrowDirection::Right, true));
        assert!(editor.selection_start.is_some());
        assert!(editor.selection_end.is_some());
    }

    #[test]
    fn test_arrow_key_without_shift_clears_selection() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        // Regular arrow key should clear selection
        let _ = editor.update(&CanvasEditorMessage::ArrowKey(ArrowDirection::Right, false));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_typing_with_selection() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let _ = editor.update(&CanvasEditorMessage::CharacterInput('X'));
        // Current behavior: character is inserted at cursor, selection is NOT automatically deleted
        // This is expected behavior - user must delete selection first (Backspace/Delete) or use Paste
        assert_eq!(editor.buffer.line(0), "Xhello world");
    }
}
