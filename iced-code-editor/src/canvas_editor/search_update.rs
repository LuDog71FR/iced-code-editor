//! Message handling for the search/replace dialog and the go-to-line dialog.

use iced::Task;
use iced::widget::operation::{focus, select_all};

use super::{CodeEditor, Message};
use crate::canvas_editor::editing::command::{
    Command, CompositeCommand, ReplaceTextCommand,
};

impl CodeEditor {
    /// Handles opening the search dialog.
    ///
    /// # Arguments
    ///
    /// * `replace` - `true` to open the search-and-replace dialog, `false`
    ///   for search-only
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses and selects all in the search input
    pub(crate) fn handle_open_search(
        &mut self,
        replace: bool,
    ) -> Task<Message> {
        self.goto_line_state.close();
        if replace {
            self.search_state.open_replace();
        } else {
            self.search_state.open_search();
        }
        if !self.search_state.query.is_empty() {
            self.search_state.update_matches(&self.buffer);
            self.search_state
                .select_match_near_cursor(self.cursors.primary_position());
        }
        self.overlay_cache.clear();

        // Focus the search input and select all text if any
        Task::batch([
            focus(self.search_state.search_input_id.clone()),
            select_all(self.search_state.search_input_id.clone()),
        ])
    }

    /// Handles closing the search dialog.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_close_search_msg(&mut self) -> Task<Message> {
        // Escape with multiple cursors and no open search: collapse to primary cursor
        if self.cursors.is_multi() && !self.search_state.is_open {
            self.cursors.remove_all_but_primary();
            self.overlay_cache.clear();
            return Task::none();
        }
        self.search_state.close();
        self.overlay_cache.clear();
        Task::none()
    }

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

    /// Handles search query text changes.
    ///
    /// # Arguments
    ///
    /// * `query` - The new search query
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to first match if any
    pub(crate) fn handle_search_query_changed_msg(
        &mut self,
        query: &str,
    ) -> Task<Message> {
        self.search_state.set_query(query.to_string(), &self.buffer);
        self.overlay_cache.clear();

        // Move cursor to first match if any
        if let Some(match_pos) = self.search_state.current_match() {
            self.cursors.primary_mut().position =
                (match_pos.line, match_pos.col);
            self.clear_selection();
            return self.scroll_to_cursor();
        }
        Task::none()
    }

    /// Handles replace query text changes.
    ///
    /// # Arguments
    ///
    /// * `replace_text` - The new replacement text
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_replace_query_changed_msg(
        &mut self,
        replace_text: &str,
    ) -> Task<Message> {
        self.search_state.set_replace_with(replace_text.to_string());
        Task::none()
    }

    /// Handles toggling case-sensitive search.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to first match if any
    pub(crate) fn handle_toggle_case_sensitive_msg(&mut self) -> Task<Message> {
        self.search_state.toggle_case_sensitive(&self.buffer);
        self.overlay_cache.clear();

        // Move cursor to first match if any
        if let Some(match_pos) = self.search_state.current_match() {
            self.cursors.primary_mut().position =
                (match_pos.line, match_pos.col);
            self.clear_selection();
            return self.scroll_to_cursor();
        }
        Task::none()
    }

    /// Handles finding the next or previous match.
    ///
    /// # Arguments
    ///
    /// * `forward` - `true` to move to the next match, `false` for the
    ///   previous match
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to the matched position if any
    pub(crate) fn handle_find_match(&mut self, forward: bool) -> Task<Message> {
        if !self.search_state.matches.is_empty() {
            if forward {
                self.search_state.next_match();
            } else {
                self.search_state.previous_match();
            }
            if let Some(match_pos) = self.search_state.current_match() {
                self.cursors.primary_mut().position =
                    (match_pos.line, match_pos.col);
                self.clear_selection();
                self.overlay_cache.clear();
                return self.scroll_to_cursor();
            }
        }
        Task::none()
    }

    /// Handles replacing the current match and moving to the next.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to the next match if any
    pub(crate) fn handle_replace_next_msg(&mut self) -> Task<Message> {
        // Replace current match and move to next
        if let Some(match_pos) = self.search_state.current_match() {
            let query_len = self.search_state.query.chars().count();
            let replace_text = self.search_state.replace_with.clone();

            // Create and execute replace command
            let pos = self.cursors.primary_position();
            let mut cmd = ReplaceTextCommand::new(
                &self.buffer,
                (match_pos.line, match_pos.col),
                query_len,
                replace_text,
                pos,
            );
            let mut cursor_pos = pos;
            cmd.execute(&mut self.buffer, &mut cursor_pos);
            self.cursors.primary_mut().position = cursor_pos;
            self.history.push(Box::new(cmd));

            // The replacement starts at the matched line; invalidate highlight
            // from there regardless of where the cursor moved next.
            self.pre_edit_line = self.pre_edit_line.min(match_pos.line);
            self.pre_edit_last_line =
                self.pre_edit_last_line.max(match_pos.line);

            self.clear_selection();
            self.finish_edit_operation();

            // Move to the closest remaining match after the replacement.
            if !self.search_state.matches.is_empty()
                && let Some(next_match) = self.search_state.current_match()
            {
                self.cursors.primary_mut().position =
                    (next_match.line, next_match.col);
            }

            return self.scroll_to_cursor();
        }
        Task::none()
    }

    /// Handles replacing all matches.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor after replacement
    pub(crate) fn handle_replace_all_msg(&mut self) -> Task<Message> {
        // Perform a fresh search to find ALL matches (ignoring the display limit)
        let all_matches = super::search::find_matches(
            &self.buffer,
            &self.search_state.query,
            self.search_state.case_sensitive,
            None, // No limit for Replace All
        );

        if !all_matches.is_empty() {
            let query_len = self.search_state.query.chars().count();
            let replace_text = self.search_state.replace_with.clone();

            // Create composite command for undo
            let mut composite = CompositeCommand::new();

            // Process matches in reverse order (to preserve positions)
            for match_pos in all_matches.iter().rev() {
                let pos = self.cursors.primary_position();
                let cmd = ReplaceTextCommand::new(
                    &self.buffer,
                    (match_pos.line, match_pos.col),
                    query_len,
                    replace_text.clone(),
                    pos,
                );
                composite.add(Box::new(cmd));
            }

            // Execute all replacements
            let mut cursor_pos = self.cursors.primary_position();
            composite.execute(&mut self.buffer, &mut cursor_pos);
            self.cursors.primary_mut().position = cursor_pos;
            self.history.push(Box::new(composite));

            // Replace All touches matches anywhere in the document, so reset
            // the highlight cache entirely.
            self.pre_edit_line = 0;
            self.pre_edit_last_line = usize::MAX;

            self.clear_selection();
            self.finish_edit_operation();
            self.scroll_to_cursor()
        } else {
            Task::none()
        }
    }

    /// Handles Tab/Shift+Tab key in the search dialog (cycles field focus).
    ///
    /// # Arguments
    ///
    /// * `forward` - `true` to cycle forward (Search → Replace → Search),
    ///   `false` to cycle backward (Replace → Search → Replace)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses the newly focused field
    pub(crate) fn handle_search_dialog_tab(
        &mut self,
        forward: bool,
    ) -> Task<Message> {
        if forward {
            self.search_state.focus_next_field();
        } else {
            self.search_state.focus_previous_field();
        }

        // Focus the appropriate input based on new focused_field
        match self.search_state.focused_field {
            crate::canvas_editor::search::SearchFocusedField::Search => {
                focus(self.search_state.search_input_id.clone())
            }
            crate::canvas_editor::search::SearchFocusedField::Replace => {
                focus(self.search_state.replace_input_id.clone())
            }
        }
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

    #[test]
    fn test_open_search_replace_opens_in_replace_mode() {
        let mut editor = CodeEditor::new("hello", "txt");
        let _ = editor.update(&Message::OpenSearchReplace);
        assert!(editor.search_state.is_open);
        assert!(editor.search_state.is_replace_mode);
    }

    #[test]
    fn test_open_search_opens_in_search_only_mode() {
        let mut editor = CodeEditor::new("hello", "txt");
        let _ = editor.update(&Message::OpenSearch);
        assert!(editor.search_state.is_open);
        assert!(!editor.search_state.is_replace_mode);
    }

    #[test]
    fn test_find_previous_moves_to_previous_match() {
        let mut editor = CodeEditor::new("foo bar foo baz foo", "txt");
        editor.search_state.open_search();
        editor.search_state.set_query("foo".to_owned(), &editor.buffer);
        assert_eq!(editor.search_state.current_match_index, Some(0));

        let _ = editor.update(&Message::FindPrevious);
        // Wraps backward from the first match to the last.
        assert_eq!(editor.search_state.current_match_index, Some(2));
    }

    #[test]
    fn test_search_dialog_shift_tab_cycles_focus_backward() {
        let mut editor = CodeEditor::new("hello", "txt");
        editor.search_state.open_replace();
        assert_eq!(
            editor.search_state.focused_field,
            crate::canvas_editor::search::SearchFocusedField::Search
        );

        let _ = editor.update(&Message::SearchDialogShiftTab);
        assert_eq!(
            editor.search_state.focused_field,
            crate::canvas_editor::search::SearchFocusedField::Replace
        );
    }
}
