//! Message handling for the go-to-line dialog.

use iced::Task;
use iced::widget::operation::{focus, select_all};

use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Opens the go-to-line input and selects the current one-based line.
    pub(crate) fn handle_open_goto_line_msg(&mut self) -> Task<Message> {
        self.search_state.close();
        self.goto_line_state.open(self.cursors.primary_position().0);
        self.overlay_cache.clear();

        Task::batch([
            focus(self.goto_line_state.input_id.clone()),
            select_all(self.goto_line_state.input_id.clone()),
        ])
    }

    /// Closes the go-to-line input without moving the cursor.
    pub(crate) fn handle_close_goto_line_msg(&mut self) -> Task<Message> {
        self.goto_line_state.close();
        self.overlay_cache.clear();
        Task::none()
    }

    /// Updates the one-based line number entered by the user.
    pub(crate) fn handle_goto_line_changed_msg(
        &mut self,
        query: &str,
    ) -> Task<Message> {
        self.goto_line_state.query = query.to_string();
        Task::none()
    }

    /// Moves to the submitted one-based line and closes the input.
    pub(crate) fn handle_submit_goto_line_msg(&mut self) -> Task<Message> {
        let Some(one_based_line) = self.goto_line_state.target_line() else {
            return Task::none();
        };

        let target_line = one_based_line
            .saturating_sub(1)
            .min(self.buffer.line_count().saturating_sub(1));
        while self.hidden_lines_set().contains(&target_line) {
            let collapsed_count = self.collapsed_folds.len();
            self.unfold_at(target_line);
            if self.collapsed_folds.len() == collapsed_count {
                break;
            }
        }

        self.goto_line_state.close();
        self.handle_goto_position(target_line, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_goto_line_prefills_current_one_based_line() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        editor.cursors.primary_mut().position = (1, 2);
        editor.search_state.open_search();

        let _ = editor.update(&Message::OpenGotoLine);

        assert!(editor.goto_line_state.is_open);
        assert_eq!(editor.goto_line_state.query, "2");
        assert!(!editor.search_state.is_open);
    }

    #[test]
    fn test_submit_goto_line_moves_to_one_based_line_and_closes_dialog() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        let _ = editor.update(&Message::OpenGotoLine);
        let _ = editor.update(&Message::GotoLineChanged("3".to_string()));

        let _ = editor.update(&Message::SubmitGotoLine);

        assert_eq!(editor.cursors.primary_position(), (2, 0));
        assert!(!editor.goto_line_state.is_open);
    }

    #[test]
    fn test_submit_goto_line_clamps_to_last_line() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        let _ = editor.update(&Message::OpenGotoLine);
        let _ = editor.update(&Message::GotoLineChanged("99".to_string()));

        let _ = editor.update(&Message::SubmitGotoLine);

        assert_eq!(editor.cursors.primary_position(), (2, 0));
        assert!(!editor.goto_line_state.is_open);
    }

    #[test]
    fn test_submit_goto_line_reveals_folded_target() {
        let mut editor =
            CodeEditor::new("root\n    child\n        nested\ntail", "rs");
        editor.fold_all();
        assert!(editor.hidden_lines_set().contains(&1));
        let _ = editor.update(&Message::OpenGotoLine);
        let _ = editor.update(&Message::GotoLineChanged("2".to_string()));

        let _ = editor.update(&Message::SubmitGotoLine);

        assert_eq!(editor.cursors.primary_position(), (1, 0));
        assert!(!editor.hidden_lines_set().contains(&1));
    }

    #[test]
    fn test_submit_goto_line_keeps_dialog_open_for_invalid_input() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        editor.cursors.primary_mut().position = (1, 1);
        let _ = editor.update(&Message::OpenGotoLine);
        let _ = editor.update(&Message::GotoLineChanged("invalid".to_string()));

        let _ = editor.update(&Message::SubmitGotoLine);

        assert_eq!(editor.cursors.primary_position(), (1, 1));
        assert!(editor.goto_line_state.is_open);
    }
}
