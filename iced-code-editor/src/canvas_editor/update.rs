//! Message handling and update logic.

use iced::Task;

use super::command::{
    Command, DeleteCharCommand, DeleteForwardCommand, InsertCharCommand,
    InsertNewlineCommand,
};
use super::{CURSOR_BLINK_INTERVAL, CodeEditor, Message};

impl CodeEditor {
    /// Updates the editor state based on messages and returns scroll commands.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to process
    ///
    /// # Returns
    ///
    /// A Task that may contain scroll commands to keep cursor visible
    pub fn update(&mut self, message: &Message) -> Task<Message> {
        match message {
            Message::CharacterInput(ch) => {
                // Start grouping if not already grouping (for smart undo)
                if !self.is_grouping {
                    self.history.begin_group("Typing");
                    self.is_grouping = true;
                }

                let (line, col) = self.cursor;
                let mut cmd =
                    InsertCharCommand::new(line, col, *ch, self.cursor);
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::Backspace => {
                // End grouping on backspace (separate from typing)
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // Check if there's a selection - if so, delete it instead
                if self.selection_start.is_some()
                    && self.selection_end.is_some()
                {
                    self.delete_selection();
                    self.reset_cursor_blink();
                    self.cache.clear();
                    return self.scroll_to_cursor();
                }

                // No selection - perform normal backspace
                let (line, col) = self.cursor;
                let mut cmd = DeleteCharCommand::new(
                    &self.buffer,
                    line,
                    col,
                    self.cursor,
                );
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Delete => {
                // End grouping on delete
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // Check if there's a selection - if so, delete it instead
                if self.selection_start.is_some()
                    && self.selection_end.is_some()
                {
                    self.delete_selection();
                    self.reset_cursor_blink();
                    self.cache.clear();
                    return self.scroll_to_cursor();
                }

                // No selection - perform normal forward delete
                let (line, col) = self.cursor;
                let mut cmd = DeleteForwardCommand::new(
                    &self.buffer,
                    line,
                    col,
                    self.cursor,
                );
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::Enter => {
                // End grouping on enter
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                let (line, col) = self.cursor;
                let mut cmd = InsertNewlineCommand::new(line, col, self.cursor);
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Tab => {
                // Insert 4 spaces for Tab
                // Start grouping if not already grouping
                if !self.is_grouping {
                    self.history.begin_group("Tab");
                    self.is_grouping = true;
                }

                let (line, col) = self.cursor;
                // Insert 4 spaces
                for i in 0..4 {
                    let current_col = col + i;
                    let mut cmd = InsertCharCommand::new(
                        line,
                        current_col,
                        ' ',
                        (line, current_col),
                    );
                    cmd.execute(&mut self.buffer, &mut self.cursor);
                    self.history.push(Box::new(cmd));
                }

                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::ArrowKey(direction, shift_pressed) => {
                // End grouping on navigation
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

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
            Message::MouseClick(point) => {
                // End grouping on mouse click
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                self.handle_mouse_click(*point);
                self.reset_cursor_blink();
                // Clear selection on click
                self.clear_selection();
                self.is_dragging = true;
                self.selection_start = Some(self.cursor);
                Task::none()
            }
            Message::MouseDrag(point) => {
                if self.is_dragging {
                    self.handle_mouse_drag(*point);
                    self.cache.clear();
                }
                Task::none()
            }
            Message::MouseRelease => {
                self.is_dragging = false;
                Task::none()
            }
            Message::Copy => self.copy_selection(),
            Message::Paste(text) => {
                // End grouping on paste
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // If text is empty, we need to read from clipboard
                if text.is_empty() {
                    // Return a task that reads clipboard and chains to paste
                    iced::clipboard::read().and_then(|clipboard_text| {
                        Task::done(Message::Paste(clipboard_text))
                    })
                } else {
                    // We have the text, paste it
                    self.paste_text(text);
                    self.cache.clear();
                    self.scroll_to_cursor()
                }
            }
            Message::DeleteSelection => {
                // End grouping on delete selection
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // Delete selected text
                self.delete_selection();
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Tick => {
                // Handle cursor blinking
                if self.last_blink.elapsed() >= CURSOR_BLINK_INTERVAL {
                    self.cursor_visible = !self.cursor_visible;
                    self.last_blink = std::time::Instant::now();
                    self.cache.clear();
                }
                Task::none()
            }
            Message::PageUp => {
                self.page_up();
                self.reset_cursor_blink();
                self.scroll_to_cursor()
            }
            Message::PageDown => {
                self.page_down();
                self.reset_cursor_blink();
                self.scroll_to_cursor()
            }
            Message::Home(shift_pressed) => {
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
            Message::End(shift_pressed) => {
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
            Message::CtrlHome => {
                // Move cursor to the beginning of the document
                self.clear_selection();
                self.cursor = (0, 0);
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::CtrlEnd => {
                // Move cursor to the end of the document
                self.clear_selection();
                let last_line = self.buffer.line_count().saturating_sub(1);
                let last_col = self.buffer.line_len(last_line);
                self.cursor = (last_line, last_col);
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Scrolled(viewport) => {
                // Track viewport scroll position, height, and width
                self.viewport_scroll = viewport.absolute_offset().y;
                let new_height = viewport.bounds().height;
                let new_width = viewport.bounds().width;
                // Clear cache when viewport dimensions change significantly
                // to ensure proper redraw (e.g., window resize)
                if (self.viewport_height - new_height).abs() > 1.0
                    || (self.viewport_width - new_width).abs() > 1.0
                {
                    self.cache.clear();
                }
                self.viewport_height = new_height;
                self.viewport_width = new_width;
                Task::none()
            }
            Message::Undo => {
                // End any current grouping before undoing
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                if self.history.undo(&mut self.buffer, &mut self.cursor) {
                    self.clear_selection();
                    self.reset_cursor_blink();
                    self.cache.clear();
                    self.scroll_to_cursor()
                } else {
                    Task::none()
                }
            }
            Message::Redo => {
                if self.history.redo(&mut self.buffer, &mut self.cursor) {
                    self.clear_selection();
                    self.reset_cursor_blink();
                    self.cache.clear();
                    self.scroll_to_cursor()
                } else {
                    Task::none()
                }
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
        let editor = CodeEditor::new("line1\nline2", "py");
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_home_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 5); // Move to middle of line
        let _ = editor.update(&Message::Home(false));
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_end_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 0);
        let _ = editor.update(&Message::End(false));
        assert_eq!(editor.cursor, (0, 11)); // Length of "hello world"
    }

    #[test]
    fn test_arrow_key_with_shift_creates_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 0);

        // Shift+Right should start selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, true));
        assert!(editor.selection_start.is_some());
        assert!(editor.selection_end.is_some());
    }

    #[test]
    fn test_arrow_key_without_shift_clears_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        // Regular arrow key should clear selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, false));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_typing_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let _ = editor.update(&Message::CharacterInput('X'));
        // Current behavior: character is inserted at cursor, selection is NOT automatically deleted
        // This is expected behavior - user must delete selection first (Backspace/Delete) or use Paste
        assert_eq!(editor.buffer.line(0), "Xhello world");
    }

    #[test]
    fn test_ctrl_home() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (2, 5); // Start at line 3, column 5
        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursor, (0, 0)); // Should move to beginning of document
    }

    #[test]
    fn test_ctrl_end() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (0, 0); // Start at beginning
        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursor, (2, 5)); // Should move to end of last line (line3 has 5 chars)
    }

    #[test]
    fn test_ctrl_home_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (2, 5);
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((2, 5));

        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursor, (0, 0));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_ctrl_end_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (0, 0);
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((1, 3));

        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursor, (2, 5));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_delete_selection_message() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 0);
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.line(0), " world");
        assert_eq!(editor.cursor, (0, 0));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_delete_selection_multiline() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (0, 2);
        editor.selection_start = Some((0, 2));
        editor.selection_end = Some((2, 2));

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.line(0), "line3");
        assert_eq!(editor.cursor, (0, 2));
        assert_eq!(editor.selection_start, None);
    }

    #[test]
    fn test_delete_selection_no_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 5);

        let _ = editor.update(&Message::DeleteSelection);
        // Should do nothing if there's no selection
        assert_eq!(editor.buffer.line(0), "hello world");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_char_insert() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type a character
        let _ = editor.update(&Message::CharacterInput('!'));
        assert_eq!(editor.buffer.line(0), "hello!");
        assert_eq!(editor.cursor, (0, 6));

        // Undo should remove it (but first end the grouping)
        editor.history.end_group();
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_redo_char_insert() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type a character
        let _ = editor.update(&Message::CharacterInput('!'));
        editor.history.end_group();

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");

        // Redo
        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "hello!");
        assert_eq!(editor.cursor, (0, 6));
    }

    #[test]
    fn test_undo_backspace() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Backspace
        let _ = editor.update(&Message::Backspace);
        assert_eq!(editor.buffer.line(0), "hell");
        assert_eq!(editor.cursor, (0, 4));

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_newline() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 5);

        // Insert newline
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.buffer.line(1), " world");
        assert_eq!(editor.cursor, (1, 0));

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello world");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_grouped_typing() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type multiple characters (they should be grouped)
        let _ = editor.update(&Message::CharacterInput(' '));
        let _ = editor.update(&Message::CharacterInput('w'));
        let _ = editor.update(&Message::CharacterInput('o'));
        let _ = editor.update(&Message::CharacterInput('r'));
        let _ = editor.update(&Message::CharacterInput('l'));
        let _ = editor.update(&Message::CharacterInput('d'));

        assert_eq!(editor.buffer.line(0), "hello world");

        // End the group
        editor.history.end_group();

        // Single undo should remove all grouped characters
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_navigation_ends_grouping() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type a character (starts grouping)
        let _ = editor.update(&Message::CharacterInput('!'));
        assert!(editor.is_grouping);

        // Move cursor (ends grouping)
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Left, false));
        assert!(!editor.is_grouping);

        // Type another character (starts new group)
        let _ = editor.update(&Message::CharacterInput('?'));
        assert!(editor.is_grouping);

        editor.history.end_group();

        // Two separate undo operations
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello!");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
    }

    #[test]
    fn test_multiple_undo_redo() {
        let mut editor = CodeEditor::new("a", "py");
        editor.cursor = (0, 1);

        // Make several changes
        let _ = editor.update(&Message::CharacterInput('b'));
        editor.history.end_group();

        let _ = editor.update(&Message::CharacterInput('c'));
        editor.history.end_group();

        let _ = editor.update(&Message::CharacterInput('d'));
        editor.history.end_group();

        assert_eq!(editor.buffer.line(0), "abcd");

        // Undo all
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "abc");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "ab");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "a");

        // Redo all
        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "ab");

        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "abc");

        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "abcd");
    }

    #[test]
    fn test_delete_key_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));
        editor.cursor = (0, 5);

        let _ = editor.update(&Message::Delete);

        assert_eq!(editor.buffer.line(0), " world");
        assert_eq!(editor.cursor, (0, 0));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_delete_key_without_selection() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 0);

        let _ = editor.update(&Message::Delete);

        // Should delete the 'h'
        assert_eq!(editor.buffer.line(0), "ello");
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_backspace_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 6));
        editor.selection_end = Some((0, 11));
        editor.cursor = (0, 11);

        let _ = editor.update(&Message::Backspace);

        assert_eq!(editor.buffer.line(0), "hello ");
        assert_eq!(editor.cursor, (0, 6));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_backspace_without_selection() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        let _ = editor.update(&Message::Backspace);

        // Should delete the 'o'
        assert_eq!(editor.buffer.line(0), "hell");
        assert_eq!(editor.cursor, (0, 4));
    }

    #[test]
    fn test_delete_multiline_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.selection_start = Some((0, 2));
        editor.selection_end = Some((2, 2));
        editor.cursor = (2, 2);

        let _ = editor.update(&Message::Delete);

        assert_eq!(editor.buffer.line(0), "line3");
        assert_eq!(editor.cursor, (0, 2));
        assert_eq!(editor.selection_start, None);
    }
}
