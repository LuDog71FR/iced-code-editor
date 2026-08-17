//! Message handling for the search/replace dialog.

use iced::Task;
use iced::widget::operation::{focus, select_all};

use crate::canvas_editor::editing::command::{
    Command, CompositeCommand, ReplaceTextCommand,
};
use crate::canvas_editor::{CodeEditor, Message};

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
        let all_matches = super::find_matches(
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
            super::SearchFocusedField::Search => {
                focus(self.search_state.search_input_id.clone())
            }
            super::SearchFocusedField::Replace => {
                focus(self.search_state.replace_input_id.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            super::super::SearchFocusedField::Search
        );

        let _ = editor.update(&Message::SearchDialogShiftTab);
        assert_eq!(
            editor.search_state.focused_field,
            super::super::SearchFocusedField::Replace
        );
    }

    // =========================================================================
    // Replace
    // =========================================================================
    //
    // `handle_replace_next_msg` and `handle_replace_all_msg` are the only two
    // handlers in this file that mutate the buffer and push undo commands, and
    // both depend on invariants that are invisible at the call site: Replace
    // All bypasses the `MAX_MATCHES` display limit and walks its matches in
    // reverse so earlier replacements can't shift later positions, and Replace
    // Next re-reads `current_match()` only after `finish_edit_operation` has
    // refreshed the match list.

    /// Opens the replace dialog with `query`/`replace_with` already filled in,
    /// which is the state the two Replace handlers actually run against.
    ///
    /// The dialog must be open: `refresh_search_matches_if_needed` is gated on
    /// `search_matches_visible()`, so a replace driven against a closed dialog
    /// would not re-run the search afterwards.
    fn replace_editor(
        content: &str,
        query: &str,
        replace_with: &str,
    ) -> CodeEditor {
        let mut editor = CodeEditor::new(content, "txt");
        editor.search_state.open_replace();
        editor.search_state.set_query(query.to_owned(), &editor.buffer);
        editor.search_state.set_replace_with(replace_with.to_owned());
        editor
    }

    #[test]
    fn test_replace_all_replaces_every_match_across_lines() {
        let mut editor =
            replace_editor("foo one\ntwo foo\nfoo foo", "foo", "bar");
        assert_eq!(editor.search_state.match_count(), 4);

        let _ = editor.update(&Message::ReplaceAll);

        assert_eq!(editor.buffer.to_string(), "bar one\ntwo bar\nbar bar");
        // The query is gone, so the refreshed match list is empty.
        assert_eq!(editor.search_state.match_count(), 0);
    }

    #[test]
    fn test_replace_all_is_a_single_undo_step() {
        let mut editor =
            replace_editor("foo one\ntwo foo\nfoo foo", "foo", "bar");

        let _ = editor.update(&Message::ReplaceAll);
        assert_eq!(editor.buffer.to_string(), "bar one\ntwo bar\nbar bar");
        assert_eq!(editor.history.undo_count(), 1);

        // All four replacements are wrapped in one `CompositeCommand`, so a
        // single undo restores the whole document rather than one match.
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "foo one\ntwo foo\nfoo foo");
        assert!(!editor.can_undo());
    }

    #[test]
    fn test_replace_all_handles_a_replacement_containing_the_query() {
        let mut editor = replace_editor("foo foo", "foo", "foofoo");

        let _ = editor.update(&Message::ReplaceAll);

        // Each match is replaced exactly once. Re-scanning the replaced text
        // would grow the buffer without bound, and walking the matches
        // forward would corrupt the later positions as the line grows.
        assert_eq!(editor.buffer.to_string(), "foofoo foofoo");
    }

    #[test]
    fn test_replace_all_shrinking_replacement_keeps_later_matches_aligned() {
        // Replacing right-to-left matters most when the replacement is a
        // different length than the query: a forward walk would leave every
        // match after the first pointing at a stale column.
        let mut editor = replace_editor("aXbXcXd", "X", "");

        let _ = editor.update(&Message::ReplaceAll);

        assert_eq!(editor.buffer.to_string(), "abcd");
    }

    #[test]
    fn test_replace_all_ignores_the_display_match_limit() {
        // `update_matches` caps the stored matches at `MAX_MATCHES` for the
        // dialog's counter, but Replace All re-runs the search with no limit
        // so it cannot silently skip the tail of a large document.
        let line_count = super::super::MAX_MATCHES + 10;
        let content = vec!["foo"; line_count].join("\n");
        let mut editor = replace_editor(&content, "foo", "bar");
        assert_eq!(
            editor.search_state.match_count(),
            super::super::MAX_MATCHES
        );

        let _ = editor.update(&Message::ReplaceAll);

        assert!(
            !editor.buffer.to_string().contains("foo"),
            "every match must be replaced, not just the first MAX_MATCHES"
        );
    }

    #[test]
    fn test_replace_all_with_no_match_changes_nothing() {
        let mut editor = replace_editor("one two", "absent", "x");

        let _ = editor.update(&Message::ReplaceAll);

        assert_eq!(editor.buffer.to_string(), "one two");
        // Nothing was pushed, so there is no empty entry to undo past.
        assert!(!editor.can_undo());
    }

    #[test]
    fn test_replace_next_replaces_only_the_current_match() {
        let mut editor = replace_editor("foo bar foo", "foo", "baz");
        assert_eq!(editor.search_state.current_match_index, Some(0));

        let _ = editor.update(&Message::ReplaceNext);

        assert_eq!(editor.buffer.to_string(), "baz bar foo");
        // The remaining match is found again by the post-edit refresh.
        assert_eq!(editor.search_state.match_count(), 1);
    }

    #[test]
    fn test_replace_next_moves_the_cursor_onto_the_following_match() {
        let mut editor = replace_editor("foo bar foo", "foo", "baz");

        let _ = editor.update(&Message::ReplaceNext);

        // After the edit, `finish_edit_operation` refreshes the match list;
        // the handler then re-reads `current_match()` and parks the cursor on
        // the surviving "foo" at column 8, ready for the next Replace.
        // (`expect` is denied workspace-wide, hence `is_some_and`.)
        let next = editor.search_state.current_match();
        assert!(
            next.is_some_and(|found| (found.line, found.col) == (0, 8)),
            "expected the surviving match at (0, 8), got {next:?}"
        );
        assert_eq!(editor.cursors.primary_position(), (0, 8));
    }

    #[test]
    fn test_replace_next_is_undoable_one_match_at_a_time() {
        let mut editor = replace_editor("foo bar foo", "foo", "baz");

        let _ = editor.update(&Message::ReplaceNext);
        let _ = editor.update(&Message::ReplaceNext);
        assert_eq!(editor.buffer.to_string(), "baz bar baz");
        assert_eq!(editor.history.undo_count(), 2);

        // Unlike Replace All, each Replace Next is its own history entry.
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "baz bar foo");
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "foo bar foo");
    }

    #[test]
    fn test_replace_next_with_no_match_changes_nothing() {
        let mut editor = replace_editor("one two", "absent", "x");
        assert_eq!(editor.search_state.current_match(), None);

        let _ = editor.update(&Message::ReplaceNext);

        assert_eq!(editor.buffer.to_string(), "one two");
        assert!(!editor.can_undo());
    }
}
