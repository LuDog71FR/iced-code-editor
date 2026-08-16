//! Undo and redo message handlers.

use iced::Task;

use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Handles undo operations.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor if undo succeeded
    pub(crate) fn handle_undo_msg(&mut self) -> Task<Message> {
        // End any current grouping before undoing
        self.end_grouping_if_active();

        let mut cursor_pos = self.cursors.primary_position();
        if self.history.undo(&mut self.buffer, &mut cursor_pos) {
            self.cursors.primary_mut().position = cursor_pos;
            self.clear_selection();
            // An undone command (especially a composite like "Replace All") may
            // touch lines anywhere in the document, so reset the highlight cache
            // entirely rather than trusting the cursor as the change origin.
            self.pre_edit_line = 0;
            self.pre_edit_last_line = usize::MAX;
            self.finish_edit_operation();
            self.scroll_to_cursor()
        } else {
            Task::none()
        }
    }

    /// Handles redo operations.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor if redo succeeded
    pub(crate) fn handle_redo_msg(&mut self) -> Task<Message> {
        let mut cursor_pos = self.cursors.primary_position();
        if self.history.redo(&mut self.buffer, &mut cursor_pos) {
            self.cursors.primary_mut().position = cursor_pos;
            self.clear_selection();
            // A redone command may touch lines anywhere; reset the highlight
            // cache entirely (see `handle_undo_msg`).
            self.pre_edit_line = 0;
            self.pre_edit_last_line = usize::MAX;
            self.finish_edit_operation();
            self.scroll_to_cursor()
        } else {
            Task::none()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_char_insert() {
        let mut editor = CodeEditor::new("hello", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().position = (0, 5);

        // Type a character
        let _ = editor.update(&Message::CharacterInput('!'));
        assert_eq!(editor.buffer.line(0), "hello!");
        assert_eq!(editor.cursors.primary_position(), (0, 6));

        // Undo should remove it (but first end the grouping)
        editor.history.end_group();
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursors.primary_position(), (0, 5));
    }

    #[test]
    fn test_undo_redo_char_insert() {
        let mut editor = CodeEditor::new("hello", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().position = (0, 5);

        // Type a character
        let _ = editor.update(&Message::CharacterInput('!'));
        editor.history.end_group();

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");

        // Redo
        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "hello!");
        assert_eq!(editor.cursors.primary_position(), (0, 6));
    }

    #[test]
    fn test_undo_backspace() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursors.primary_mut().position = (0, 5);

        // Backspace
        let _ = editor.update(&Message::Backspace);
        assert_eq!(editor.buffer.line(0), "hell");
        assert_eq!(editor.cursors.primary_position(), (0, 4));

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursors.primary_position(), (0, 5));
    }

    #[test]
    fn test_undo_backspace_line_merge() {
        // Backspace at column 0 merges two lines; undo must restore both
        // without duplicating the merged line.
        let mut editor = CodeEditor::new("hello\nworld", "py");
        editor.cursors.set_single((1, 0));

        let _ = editor.update(&Message::Backspace);
        assert_eq!(editor.content(), "helloworld");
        assert_eq!(editor.cursors.primary_position(), (0, 5));

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.content(), "hello\nworld");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
    }

    #[test]
    fn test_undo_newline() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 5);

        // Insert newline
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.buffer.line(1), " world");
        assert_eq!(editor.cursors.primary_position(), (1, 0));

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello world");
        assert_eq!(editor.cursors.primary_position(), (0, 5));
    }

    #[test]
    fn test_undo_grouped_typing() {
        let mut editor = CodeEditor::new("hello", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().position = (0, 5);

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
        assert_eq!(editor.cursors.primary_position(), (0, 5));
    }

    #[test]
    fn test_multiple_undo_redo() {
        let mut editor = CodeEditor::new("a", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().position = (0, 1);

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
}
