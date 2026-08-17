//! Text input, tab, enter, and focus-navigation message handlers.

use iced::Task;

use super::{EditType, adjust_other_cursors};
use crate::canvas_editor::editing::command::{
    Command, InsertCharCommand, InsertNewlineCommand,
};
use crate::canvas_editor::{CodeEditor, IndentStyle, Message};

/// Returns the closing character auto-inserted for an opening bracket or
/// quote, or `None` if `ch` doesn't start an auto-closeable pair.
///
/// # Examples
///
/// ```text
/// assert_eq!(matching_close('('), Some(')'));
/// assert_eq!(matching_close('"'), Some('"'));
/// assert_eq!(matching_close('x'), None);
/// ```
fn matching_close(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}

/// Returns `true` if `ch` is one of the closing brackets/quotes managed by
/// auto-close (`)`, `]`, `}`, `"`, `'`).
fn is_closing_char(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}' | '"' | '\'')
}

impl CodeEditor {
    /// Handles character input message operations.
    ///
    /// Inserts a character at the current cursor position and adds it to the
    /// undo history. Characters are grouped together for smart undo.
    /// Only processes input when the editor has active focus and is not locked.
    ///
    /// When [`auto_close_brackets`](CodeEditor::auto_close_brackets) is enabled:
    /// - Typing an opening bracket/quote while a selection is active wraps the
    ///   selection in the pair (surround selection) instead of replacing it.
    /// - Typing an opening bracket/quote with no selection auto-inserts the
    ///   matching closing character, leaving the cursor between the two.
    /// - Typing a closing character that already sits at the cursor "types
    ///   through" it (the cursor just steps over it) instead of duplicating it.
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to insert
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible (including
    /// horizontal scroll when wrap is disabled)
    pub(crate) fn handle_character_input_msg(
        &mut self,
        ch: char,
    ) -> Task<Message> {
        // Guard clause: only process character input if editor has focus and is not locked
        if !self.has_focus() {
            return Task::none();
        }

        // Start grouping if not already grouping (for smart undo)
        self.ensure_grouping_started();

        let has_any_selection =
            self.cursors.iter().any(|cursor| cursor.has_selection());
        let surround_close = if self.auto_close_brackets && has_any_selection {
            matching_close(ch)
        } else {
            None
        };

        if let Some(close) = surround_close {
            self.surround_selections_with_pair(ch, close);
        } else {
            // Typing replaces active selections, matching paste and IME commit
            // behavior. Keep the deletion and insertion in the same history group
            // so a single undo restores the replaced text.
            if has_any_selection {
                self.delete_selection();
            } else {
                // A plain click leaves a zero-length anchor in place (see
                // `handle_enter`); clear it so it isn't mistaken for a real
                // selection by a later edit.
                self.clear_selection();
            }

            // Multi-cursor: build a sorted index list (descending document order)
            // so that edits at higher positions don't invalidate lower positions.
            let order = self.cursors.descending_order();

            for &idx in &order {
                // Any active selection was deleted above, which also moves the
                // cursor to the original selection start and clears its anchor.
                // The current cursor position is therefore the insertion point.
                let pos = self.cursors.as_slice()[idx].position;

                if self.auto_close_brackets
                    && is_closing_char(ch)
                    && self.char_at(pos) == Some(ch)
                {
                    // An auto-inserted closer already sits here: step over it
                    // instead of inserting a duplicate.
                    self.cursors.as_mut_slice()[idx].position =
                        (pos.0, pos.1 + 1);
                    continue;
                }

                if self.auto_close_brackets
                    && let Some(close) = matching_close(ch)
                    && self.should_auto_close(pos)
                {
                    self.insert_pair_at_cursor(idx, pos, ch, close);
                    continue;
                }

                let mut cmd = InsertCharCommand::new(pos.0, pos.1, ch, pos);
                let mut cursor_pos = pos;
                cmd.execute(&mut self.buffer, &mut cursor_pos);
                self.cursors.as_mut_slice()[idx].position = cursor_pos;
                adjust_other_cursors(
                    self.cursors.as_mut_slice(),
                    idx,
                    pos.0,
                    pos.1,
                    EditType::InsertChar,
                );
                self.history.push(Box::new(cmd));
            }
        }

        self.finish_edit_operation();

        // Auto-trigger LSP completion for identifier characters and trigger characters
        if ch.is_alphanumeric() || ch == '_' || ch == '.' {
            self.lsp_flush_pending_changes();
            self.lsp_request_completion();
        }

        self.scroll_to_cursor()
    }

    /// Returns the character at `(line, col)`, or `None` if `col` is at or
    /// past the end of the line.
    fn char_at(&self, pos: (usize, usize)) -> Option<char> {
        self.buffer.line(pos.0).chars().nth(pos.1)
    }

    /// Returns `true` when auto-closing a pair at `pos` would not wrap
    /// existing text.
    ///
    /// Auto-close only triggers when the cursor is at the end of the line, or
    /// is followed by whitespace or another closing bracket/quote. This
    /// avoids inserting a spurious pair in the middle of an identifier (e.g.
    /// typing `'` inside `sn|ake`).
    fn should_auto_close(&self, pos: (usize, usize)) -> bool {
        match self.char_at(pos) {
            None => true,
            Some(next) => next.is_whitespace() || is_closing_char(next),
        }
    }

    /// Inserts `open` immediately followed by `close` at `pos` for cursor
    /// `idx`, leaving that cursor positioned between the two characters.
    ///
    /// Pushed as two grouped [`InsertCharCommand`]s (grouped with the current
    /// typing group) so each has independent, correct undo semantics — a
    /// single command spanning both chars with the cursor resting *between*
    /// them (rather than after, as `InsertTextCommand` assumes) would make
    /// `InsertTextCommand::undo`'s backward walk from the resting cursor
    /// delete the wrong character. Adjusts every other cursor for the
    /// resulting 2-character insert.
    fn insert_pair_at_cursor(
        &mut self,
        idx: usize,
        pos: (usize, usize),
        open: char,
        close: char,
    ) {
        let mut open_cmd = InsertCharCommand::new(pos.0, pos.1, open, pos);
        let mut cursor_pos = pos;
        open_cmd.execute(&mut self.buffer, &mut cursor_pos);
        self.history.push(Box::new(open_cmd));

        let mut close_cmd =
            InsertCharCommand::new(pos.0, pos.1 + 1, close, cursor_pos);
        close_cmd.execute(&mut self.buffer, &mut cursor_pos);
        self.history.push(Box::new(close_cmd));

        // Rest the cursor between the two inserted characters rather than
        // after the close char (where `close_cmd`'s own `cursor_after` puts it).
        self.cursors.as_mut_slice()[idx].position = (pos.0, pos.1 + 1);

        // Two characters were inserted at the same column: apply the
        // single-char adjustment twice so other cursors shift by 2.
        adjust_other_cursors(
            self.cursors.as_mut_slice(),
            idx,
            pos.0,
            pos.1,
            EditType::InsertChar,
        );
        adjust_other_cursors(
            self.cursors.as_mut_slice(),
            idx,
            pos.0,
            pos.1,
            EditType::InsertChar,
        );
    }

    /// Wraps every selected cursor's text in `open`/`close`, and inserts the
    /// pair at any cursor with no active selection.
    ///
    /// Cursors are processed in descending document order (by the start of
    /// their selection, or their position when unselected) so that edits at
    /// higher positions don't invalidate earlier ones. Wrapped selections
    /// keep their original direction (anchor/position) so the originally
    /// selected text stays selected between the newly inserted pair.
    fn surround_selections_with_pair(&mut self, open: char, close: char) {
        let order = self.cursors.descending_order_by_key(
            |_| true,
            |c| c.selection_range().map_or(c.position, |(s, _)| s),
        );

        for idx in order {
            let cursor = self.cursors.as_slice()[idx].clone();
            let Some((start, end)) = cursor.selection_range() else {
                self.insert_pair_at_cursor(idx, cursor.position, open, close);
                continue;
            };
            let anchor_is_start = cursor.anchor == Some(start);

            let mut open_cmd =
                InsertCharCommand::new(start.0, start.1, open, cursor.position);
            let mut cursor_pos = cursor.position;
            open_cmd.execute(&mut self.buffer, &mut cursor_pos);
            self.history.push(Box::new(open_cmd));
            adjust_other_cursors(
                self.cursors.as_mut_slice(),
                idx,
                start.0,
                start.1,
                EditType::InsertChar,
            );

            // The open char shifted everything after it on the same line by
            // one column, including `end` if the selection is single-line.
            let close_col = if end.0 == start.0 { end.1 + 1 } else { end.1 };
            let mut close_cmd =
                InsertCharCommand::new(end.0, close_col, close, cursor_pos);
            close_cmd.execute(&mut self.buffer, &mut cursor_pos);
            self.history.push(Box::new(close_cmd));
            adjust_other_cursors(
                self.cursors.as_mut_slice(),
                idx,
                end.0,
                close_col,
                EditType::InsertChar,
            );

            let new_start = (start.0, start.1 + 1);
            let new_end = (end.0, close_col);
            let cursor = &mut self.cursors.as_mut_slice()[idx];
            if anchor_is_start {
                cursor.anchor = Some(new_start);
                cursor.position = new_end;
            } else {
                cursor.anchor = Some(new_end);
                cursor.position = new_start;
            }
        }
    }

    /// Handles Tab key press (inserts 4 spaces).
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible (including
    /// horizontal scroll when wrap is disabled)
    pub(crate) fn handle_tab(&mut self) -> Task<Message> {
        self.ensure_grouping_started();

        // A plain click leaves a zero-length anchor in place (see
        // `handle_enter`); clear it so it isn't mistaken for a real selection
        // by a later edit. Tab only reaches here when there is no real
        // selection to indent (see `IndentLines`).
        self.clear_selection();

        // Multi-cursor: process in descending document order
        let order = self.cursors.descending_order();

        for &idx in &order {
            let pos = self.cursors.as_slice()[idx].position;
            match self.indent_style {
                IndentStyle::Spaces(n) => {
                    let mut cursor_pos = pos;
                    for _i in 0..n as usize {
                        let current_col = cursor_pos.1;
                        let mut cmd = InsertCharCommand::new(
                            pos.0,
                            current_col,
                            ' ',
                            cursor_pos,
                        );
                        cmd.execute(&mut self.buffer, &mut cursor_pos);
                        adjust_other_cursors(
                            self.cursors.as_mut_slice(),
                            idx,
                            pos.0,
                            current_col,
                            EditType::InsertChar,
                        );
                        self.history.push(Box::new(cmd));
                    }
                    self.cursors.as_mut_slice()[idx].position = cursor_pos;
                }
                IndentStyle::Tab => {
                    let mut cmd =
                        InsertCharCommand::new(pos.0, pos.1, '\t', pos);
                    let mut cursor_pos = pos;
                    cmd.execute(&mut self.buffer, &mut cursor_pos);
                    adjust_other_cursors(
                        self.cursors.as_mut_slice(),
                        idx,
                        pos.0,
                        pos.1,
                        EditType::InsertChar,
                    );
                    self.cursors.as_mut_slice()[idx].position = cursor_pos;
                    self.history.push(Box::new(cmd));
                }
            }
        }

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    /// Handles Shift+Tab key presses for focus navigation (when the search
    /// dialog is not open).
    ///
    /// Relinquishes this editor's own focus; the `FocusNavigationShiftTab`
    /// message is also published to the host application (see
    /// `canvas_impl.rs`), which owns moving focus to another widget.
    ///
    /// # Returns
    ///
    /// Always `Task::none()`
    pub(crate) fn handle_focus_navigation(&mut self) -> Task<Message> {
        if !self.search_state.is_open {
            self.has_canvas_focus = false;
            self.show_cursor = false;
        }

        Task::none()
    }

    /// Handles Enter key press (inserts newline).
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_enter(&mut self) -> Task<Message> {
        // Standard editing treats Enter as a boundary. In Vim Insert mode the
        // newline belongs to the same insertion session and is closed by Esc.
        let keep_vim_group = self.keep_vim_insert_group();
        if !keep_vim_group {
            self.end_grouping_if_active();
        }

        // A mouse click leaves a zero-length anchor in place so a following
        // drag can extend the selection. Enter must clear that anchor before
        // moving the caret to the new line; otherwise the inserted newline
        // becomes selected and the next typed character deletes it.
        //
        // For a real selection, Enter replaces the selected text with a
        // newline. Group both commands so one undo restores the selection.
        let replaces_selection =
            self.cursors.iter().any(|cursor| cursor.has_selection());
        if replaces_selection {
            self.ensure_grouping_started();
            self.delete_selection();
        } else {
            self.clear_selection();
        }

        // Multi-cursor: process in descending document order
        let order = self.cursors.descending_order();

        for &idx in &order {
            let pos = self.cursors.as_slice()[idx].position;

            // Copy leading whitespace of the current line to the new line (if enabled)
            let indent: String = if self.auto_indent_enabled {
                self.buffer
                    .line(pos.0)
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .collect()
            } else {
                String::new()
            };
            let indent_len = indent.chars().count();

            let mut cmd =
                InsertNewlineCommand::with_indent(pos.0, pos.1, pos, indent);
            let mut cursor_pos = pos;
            cmd.execute(&mut self.buffer, &mut cursor_pos);
            self.cursors.as_mut_slice()[idx].position = cursor_pos;
            adjust_other_cursors(
                self.cursors.as_mut_slice(),
                idx,
                pos.0,
                pos.1,
                EditType::InsertNewline { indent_len },
            );
            self.history.push(Box::new(cmd));
        }

        if replaces_selection && !keep_vim_group {
            self.end_grouping_if_active();
        }

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn focus_editor(editor: &mut CodeEditor) {
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
    }

    #[test]
    fn test_typing_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::CharacterInput('X'));
        assert_eq!(editor.buffer.line(0), "X world");
        assert_eq!(editor.cursors.primary_position(), (0, 1));
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_typing_digit_with_reversed_selection_replaces_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().anchor = Some((0, 5));
        editor.cursors.primary_mut().position = (0, 0);

        let _ = editor.update(&Message::CharacterInput('7'));
        assert_eq!(editor.buffer.line(0), "7 world");
        assert_eq!(editor.cursors.primary_position(), (0, 1));
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_typing_with_selection_undoes_as_single_edit() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::CharacterInput('X'));
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello world");
    }

    #[test]
    fn test_matching_close_pairs() {
        assert_eq!(matching_close('('), Some(')'));
        assert_eq!(matching_close('['), Some(']'));
        assert_eq!(matching_close('{'), Some('}'));
        assert_eq!(matching_close('"'), Some('"'));
        assert_eq!(matching_close('\''), Some('\''));
        assert_eq!(matching_close('a'), None);
    }

    #[test]
    fn test_is_closing_char() {
        for ch in [')', ']', '}', '"', '\''] {
            assert!(is_closing_char(ch), "{ch} should be a closing char");
        }
        assert!(!is_closing_char('('));
        assert!(!is_closing_char('a'));
    }

    #[test]
    fn test_auto_close_inserts_pair_with_cursor_between() {
        for &(open, close) in
            &[('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')]
        {
            let mut editor = CodeEditor::new("", "py");
            focus_editor(&mut editor);

            let _ = editor.update(&Message::CharacterInput(open));
            assert_eq!(
                editor.buffer.line(0),
                format!("{open}{close}"),
                "pair {open}{close}"
            );
            assert_eq!(
                editor.cursors.primary_position(),
                (0, 1),
                "pair {open}{close}"
            );
            assert!(!editor.cursors.primary().has_selection());
        }
    }

    #[test]
    fn test_auto_close_types_through_existing_closer() {
        let mut editor = CodeEditor::new("()", "py");
        focus_editor(&mut editor);
        editor.cursors.primary_mut().position = (0, 1);

        let _ = editor.update(&Message::CharacterInput(')'));
        assert_eq!(editor.buffer.line(0), "()");
        assert_eq!(editor.cursors.primary_position(), (0, 2));
    }

    #[test]
    fn test_auto_close_suppressed_before_word_char() {
        let mut editor = CodeEditor::new("snake", "py");
        focus_editor(&mut editor);
        editor.cursors.primary_mut().position = (0, 2); // sn|ake

        let _ = editor.update(&Message::CharacterInput('\''));
        assert_eq!(editor.buffer.line(0), "sn'ake");
        assert_eq!(editor.cursors.primary_position(), (0, 3));
    }

    #[test]
    fn test_auto_close_undo_removes_pair_in_one_step() {
        let mut editor = CodeEditor::new("", "py");
        focus_editor(&mut editor);

        let _ = editor.update(&Message::CharacterInput('('));
        assert_eq!(editor.buffer.line(0), "()");
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "");
    }

    #[test]
    fn test_auto_close_multi_cursor_shifts_other_cursors() {
        let mut editor = CodeEditor::new("a b", "py");
        focus_editor(&mut editor);
        editor.cursors.set_single((0, 1));
        editor.cursors.add_cursor((0, 3));

        let _ = editor.update(&Message::CharacterInput('('));
        assert_eq!(editor.buffer.line(0), "a() b()");
        assert_eq!(editor.cursors.as_slice()[0].position, (0, 2));
        assert_eq!(editor.cursors.as_slice()[1].position, (0, 6));
    }

    #[test]
    fn test_auto_close_brackets_disabled_inserts_plain_char() {
        let mut editor = CodeEditor::new("", "py");
        focus_editor(&mut editor);
        editor.set_auto_close_brackets(false);

        let _ = editor.update(&Message::CharacterInput('('));
        assert_eq!(editor.buffer.line(0), "(");
        assert_eq!(editor.cursors.primary_position(), (0, 1));
    }

    #[test]
    fn test_surround_selection_wraps_forward_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        focus_editor(&mut editor);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::CharacterInput('"'));
        assert_eq!(editor.buffer.line(0), "\"hello\" world");
        assert_eq!(editor.cursors.primary().anchor, Some((0, 1)));
        assert_eq!(editor.cursors.primary_position(), (0, 6));
        assert!(editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_surround_selection_wraps_reversed_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        focus_editor(&mut editor);
        editor.cursors.primary_mut().anchor = Some((0, 5));
        editor.cursors.primary_mut().position = (0, 0);

        let _ = editor.update(&Message::CharacterInput('('));
        assert_eq!(editor.buffer.line(0), "(hello) world");
        assert_eq!(editor.cursors.primary().anchor, Some((0, 6)));
        assert_eq!(editor.cursors.primary_position(), (0, 1));
    }

    #[test]
    fn test_surround_selection_wraps_multiline_selection() {
        let mut editor = CodeEditor::new("foo\nbar", "py");
        focus_editor(&mut editor);
        editor.cursors.primary_mut().anchor = Some((0, 1));
        editor.cursors.primary_mut().position = (1, 2);

        let _ = editor.update(&Message::CharacterInput('['));
        assert_eq!(editor.buffer.line(0), "f[oo");
        assert_eq!(editor.buffer.line(1), "ba]r");
        assert_eq!(editor.cursors.primary().anchor, Some((0, 2)));
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_surround_selection_undoes_as_single_group() {
        let mut editor = CodeEditor::new("hello world", "py");
        focus_editor(&mut editor);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::CharacterInput('"'));
        assert_eq!(editor.buffer.line(0), "\"hello\" world");
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello world");
    }

    #[test]
    fn test_surround_selection_disabled_replaces_selection_instead() {
        let mut editor = CodeEditor::new("hello world", "py");
        focus_editor(&mut editor);
        editor.set_auto_close_brackets(false);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::CharacterInput('"'));
        assert_eq!(editor.buffer.line(0), "\" world");
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_enter_no_indent() {
        let mut editor = CodeEditor::new("hello", "rs");
        editor.cursors.primary_mut().position = (0, 5);
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.buffer.line(1), "");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
    }

    #[test]
    fn test_typing_after_enter_does_not_delete_newline_from_click_anchor() {
        let mut editor = CodeEditor::new("hello", "rs");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        // A regular click starts drag tracking with a zero-length anchor.
        editor.cursors.primary_mut().position = (0, 5);
        editor.cursors.primary_mut().anchor = Some((0, 5));

        let _ = editor.update(&Message::Enter);
        let _ = editor.update(&Message::CharacterInput('X'));

        assert_eq!(editor.buffer.line_count(), 2);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.buffer.line(1), "X");
        assert_eq!(editor.cursors.primary_position(), (1, 1));
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_enter_replaces_selection_and_undo_restores_text() {
        let mut editor = CodeEditor::new("hello world", "rs");
        editor.set_auto_indent_enabled(false);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.update(&Message::Enter);

        assert_eq!(editor.buffer.line(0), "");
        assert_eq!(editor.buffer.line(1), " world");
        assert_eq!(editor.cursors.primary_position(), (1, 0));
        assert!(!editor.cursors.primary().has_selection());

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.content(), "hello world");
    }

    #[test]
    fn test_enter_auto_indent_spaces() {
        let mut editor = CodeEditor::new("    hello", "rs");
        editor.cursors.primary_mut().position = (0, 9);
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line(0), "    hello");
        assert_eq!(editor.buffer.line(1), "    ");
        assert_eq!(editor.cursors.primary_position(), (1, 4));
    }

    #[test]
    fn test_enter_auto_indent_tab() {
        let mut editor = CodeEditor::new("\thello", "rs");
        editor.cursors.primary_mut().position = (0, 6);
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line(0), "\thello");
        assert_eq!(editor.buffer.line(1), "\t");
        assert_eq!(editor.cursors.primary_position(), (1, 1));
    }

    #[test]
    fn test_enter_auto_indent_undo() {
        let mut editor = CodeEditor::new("    hello", "rs");
        editor.cursors.primary_mut().position = (0, 9);
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line_count(), 2);

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line_count(), 1);
        assert_eq!(editor.buffer.line(0), "    hello");
        assert_eq!(editor.cursors.primary_position(), (0, 9));
    }

    // =========================================================================
    // Multi-cursor tests
    // =========================================================================

    #[test]
    fn test_focus_navigation_shift_tab_loses_focus_when_search_closed() {
        let mut editor = CodeEditor::new("hello", "txt");
        focus_editor(&mut editor);
        assert!(editor.has_canvas_focus);

        let _ = editor.update(&Message::FocusNavigationShiftTab);
        assert!(!editor.has_canvas_focus);
        assert!(!editor.show_cursor);
    }

    #[test]
    fn test_focus_navigation_shift_tab_keeps_focus_when_search_open() {
        let mut editor = CodeEditor::new("hello", "txt");
        focus_editor(&mut editor);
        editor.search_state.open_search();

        let _ = editor.update(&Message::FocusNavigationShiftTab);
        assert!(editor.has_canvas_focus);
    }
}
