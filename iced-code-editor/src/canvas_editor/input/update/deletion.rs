//! Backspace, delete, and explicit-selection-delete message handlers.

use iced::Task;

use super::{EditType, adjust_other_cursors};
use crate::canvas_editor::command::{
    Command, DeleteCharCommand, DeleteForwardCommand,
};
use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Handles Backspace key press.
    ///
    /// If there's a selection, deletes the selection. Otherwise, deletes the
    /// character before the cursor.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible if selection was deleted
    pub(crate) fn handle_backspace(&mut self) -> Task<Message> {
        // End grouping on backspace (separate from typing)
        if !self.keep_vim_insert_group() {
            self.end_grouping_if_active();
        }

        // If any cursor has a selection, delete all selections first
        if self.delete_selection_if_present() {
            return self.scroll_to_cursor();
        }

        // A mouse click leaves a zero-length anchor in place so a following
        // drag can extend the selection (see `handle_enter`). Backspace must
        // clear it before moving the caret; otherwise the anchor is left
        // behind at the pre-edit position and a phantom one-character
        // selection appears next to the cursor, which the next Backspace or
        // Delete then eats instead of a single character.
        self.clear_selection();

        // Multi-cursor: process in descending document order
        let order = self.cursors.descending_order();

        for &idx in &order {
            let pos = self.cursors.as_slice()[idx].position;
            // Determine edit type for adjusting other cursors
            let edit_kind = if pos.1 > 0 {
                EditType::DeleteCharBack
            } else if pos.0 > 0 {
                let prev_line_len = self.buffer.line_len(pos.0 - 1);
                EditType::MergePrev { prev_line_len }
            } else {
                // At very start of document: nothing to delete
                continue;
            };
            let mut cmd =
                DeleteCharCommand::new(&self.buffer, pos.0, pos.1, pos);
            let mut cursor_pos = pos;
            cmd.execute(&mut self.buffer, &mut cursor_pos);
            self.cursors.as_mut_slice()[idx].position = cursor_pos;
            adjust_other_cursors(
                self.cursors.as_mut_slice(),
                idx,
                pos.0,
                pos.1,
                edit_kind,
            );
            self.history.push(Box::new(cmd));
        }

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    /// Handles Delete key press.
    ///
    /// If there's a selection, deletes the selection. Otherwise, deletes the
    /// character after the cursor.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible if selection was deleted
    pub(crate) fn handle_delete(&mut self) -> Task<Message> {
        // End grouping on delete
        if !self.keep_vim_insert_group() {
            self.end_grouping_if_active();
        }

        // If any cursor has a selection, delete all selections first
        if self.delete_selection_if_present() {
            return self.scroll_to_cursor();
        }

        // See the matching comment in `handle_backspace`: clear any
        // zero-length anchor left by a plain click before editing, so it
        // can't be mistaken for a real selection on a later edit.
        self.clear_selection();

        // Multi-cursor: process in descending document order
        let order = self.cursors.descending_order();

        for &idx in &order {
            let pos = self.cursors.as_slice()[idx].position;
            let line_len = self.buffer.line_len(pos.0);
            let edit_kind = if pos.1 < line_len {
                EditType::DeleteCharForward
            } else if pos.0 + 1 < self.buffer.line_count() {
                EditType::MergeNext { edit_line_len: line_len }
            } else {
                // At very end of document: nothing to delete
                continue;
            };
            let mut cmd =
                DeleteForwardCommand::new(&self.buffer, pos.0, pos.1, pos);
            let mut cursor_pos = pos;
            cmd.execute(&mut self.buffer, &mut cursor_pos);
            self.cursors.as_mut_slice()[idx].position = cursor_pos;
            adjust_other_cursors(
                self.cursors.as_mut_slice(),
                idx,
                pos.0,
                pos.1,
                edit_kind,
            );
            self.history.push(Box::new(cmd));
        }

        self.finish_edit_operation();
        Task::none()
    }

    /// Handles explicit selection deletion (Shift+Delete).
    ///
    /// Deletes the selected text if a selection exists.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_delete_selection(&mut self) -> Task<Message> {
        // End grouping on delete selection
        self.end_grouping_if_active();

        if self.delete_selection_if_present() {
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
    fn test_delete_selection_message() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 0);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.line(0), " world");
        assert_eq!(editor.cursors.primary_position(), (0, 0));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_delete_selection_multiline() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (0, 2);
        editor.cursors.primary_mut().anchor = Some((0, 2));
        editor.cursors.primary_mut().position = (2, 2);

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.line(0), "line3");
        assert_eq!(editor.cursors.primary_position(), (0, 2));
        assert!(editor.cursors.primary().anchor.is_none());
    }

    #[test]
    fn test_delete_selection_no_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::DeleteSelection);
        // Should do nothing if there's no selection
        assert_eq!(editor.buffer.line(0), "hello world");
        assert_eq!(editor.cursors.primary_position(), (0, 5));
    }

    #[test]
    fn test_delete_key_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::Delete);

        assert_eq!(editor.buffer.line(0), " world");
        assert_eq!(editor.cursors.primary_position(), (0, 0));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_delete_key_without_selection() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursors.primary_mut().position = (0, 0);

        let _ = editor.update(&Message::Delete);

        // Should delete the 'h'
        assert_eq!(editor.buffer.line(0), "ello");
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_backspace_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 6));
        editor.cursors.primary_mut().position = (0, 11);
        editor.cursors.primary_mut().position = (0, 11);

        let _ = editor.update(&Message::Backspace);

        assert_eq!(editor.buffer.line(0), "hello ");
        assert_eq!(editor.cursors.primary_position(), (0, 6));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_backspace_without_selection() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::Backspace);

        // Should delete the 'o'
        assert_eq!(editor.buffer.line(0), "hell");
        assert_eq!(editor.cursors.primary_position(), (0, 4));
    }

    #[test]
    fn test_delete_multiline_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().anchor = Some((0, 2));
        editor.cursors.primary_mut().position = (2, 2);
        editor.cursors.primary_mut().position = (2, 2);

        let _ = editor.update(&Message::Delete);

        assert_eq!(editor.buffer.line(0), "line3");
        assert_eq!(editor.cursors.primary_position(), (0, 2));
        assert!(editor.cursors.primary().anchor.is_none());
    }
}
