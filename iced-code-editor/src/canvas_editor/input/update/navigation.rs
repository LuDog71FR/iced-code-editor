//! Arrow-key, Home/End, Ctrl+Home/End, Page Up/Down, and goto-position message handlers.

use iced::Task;

use crate::canvas_editor::{ArrowDirection, CodeEditor, Message, VimMode};

impl CodeEditor {
    pub(crate) fn vim_accepts_insert_input(&self) -> bool {
        !self.vim_enabled || self.vim_state.mode() == VimMode::Insert
    }

    /// Handles arrow key navigation.
    ///
    /// # Arguments
    ///
    /// * `direction` - The direction of movement
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_arrow_key(
        &mut self,
        direction: ArrowDirection,
        shift_pressed: bool,
    ) -> Task<Message> {
        // End grouping on navigation
        self.end_grouping_if_active();

        if shift_pressed {
            // Set anchor on ALL cursors that don't yet have one
            for cursor in self.cursors.as_mut_slice() {
                if cursor.anchor.is_none() {
                    cursor.set_anchor();
                }
            }
            self.move_cursor(direction);
        } else {
            // Clear all selections, then move all cursors
            self.clear_selection();
            self.move_cursor(direction);
        }
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Home key press.
    ///
    /// Moves the cursor to the start of the current line.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible (including
    /// horizontal scroll back to x=0 when wrap is disabled)
    pub(crate) fn handle_home(&mut self, shift_pressed: bool) -> Task<Message> {
        if shift_pressed {
            for cursor in self.cursors.as_mut_slice() {
                if cursor.anchor.is_none() {
                    cursor.set_anchor();
                }
                cursor.position.1 = 0;
            }
        } else {
            self.clear_selection();
            for cursor in self.cursors.as_mut_slice() {
                cursor.position.1 = 0;
            }
        }
        self.cursors.sort_and_merge();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles End key press.
    ///
    /// Moves the cursor to the end of the current line.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible (including
    /// horizontal scroll to end of line when wrap is disabled)
    pub(crate) fn handle_end(&mut self, shift_pressed: bool) -> Task<Message> {
        if shift_pressed {
            for cursor in self.cursors.as_mut_slice() {
                if cursor.anchor.is_none() {
                    cursor.set_anchor();
                }
                cursor.position.1 = self.buffer.line_len(cursor.position.0);
            }
        } else {
            self.clear_selection();
            for cursor in self.cursors.as_mut_slice() {
                cursor.position.1 = self.buffer.line_len(cursor.position.0);
            }
        }
        self.cursors.sort_and_merge();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Ctrl+Home key press.
    ///
    /// Moves the cursor to the beginning of the document.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_ctrl_home(&mut self) -> Task<Message> {
        // Move cursor to the beginning of the document
        self.clear_selection();
        self.cursors.set_single((0, 0));
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Ctrl+End key press.
    ///
    /// Moves the cursor to the end of the document.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_ctrl_end(&mut self) -> Task<Message> {
        // Move cursor to the end of the document
        self.clear_selection();
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_len(last_line);
        self.cursors.set_single((last_line, last_col));
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Page Up key press.
    ///
    /// Scrolls the view up by one page.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_page_up(&mut self) -> Task<Message> {
        self.page_up();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Page Down key press.
    ///
    /// Scrolls the view down by one page.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_page_down(&mut self) -> Task<Message> {
        self.page_down();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles direct navigation to an explicit logical position.
    ///
    /// # Arguments
    ///
    /// * `line` - Target line index (0-based)
    /// * `col` - Target column index (0-based)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_goto_position(
        &mut self,
        line: usize,
        col: usize,
    ) -> Task<Message> {
        // End grouping on navigation command
        self.end_grouping_if_active();
        self.set_cursor(line, col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 5); // Move to middle of line
        let _ = editor.update(&Message::Home(false));
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_end_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 0);
        let _ = editor.update(&Message::End(false));
        assert_eq!(editor.cursors.primary_position(), (0, 11)); // Length of "hello world"
    }

    #[test]
    fn test_arrow_key_with_shift_creates_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 0);

        // Shift+Right should start selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, true));
        assert!(editor.cursors.primary().anchor.is_some());
        assert!(editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_arrow_key_without_shift_clears_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        // Regular arrow key should clear selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, false));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_ctrl_home() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (2, 5); // Start at line 3, column 5
        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursors.primary_position(), (0, 0)); // Should move to beginning of document
    }

    #[test]
    fn test_ctrl_end() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (0, 0); // Start at beginning
        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursors.primary_position(), (2, 5)); // Should move to end of last line (line3 has 5 chars)
    }

    #[test]
    fn test_ctrl_home_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (2, 5);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (2, 5);

        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursors.primary_position(), (0, 0));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_ctrl_end_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (0, 0);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (1, 3);

        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursors.primary_position(), (2, 5));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_goto_position_sets_cursor_and_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (1, 2);

        let _ = editor.update(&Message::GotoPosition(1, 3));

        assert_eq!(editor.cursors.primary_position(), (1, 3));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_goto_position_clamps_out_of_range() {
        let mut editor = CodeEditor::new("a\nbb", "py");

        let _ = editor.update(&Message::GotoPosition(99, 99));

        // Clamped to last line (index 1) and end of that line (len = 2)
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_navigation_ends_grouping() {
        let mut editor = CodeEditor::new("hello", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().position = (0, 5);

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
}
