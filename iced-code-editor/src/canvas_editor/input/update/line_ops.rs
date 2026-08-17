//! Line move/duplicate and comment-toggle message handlers.

use iced::Task;

use crate::canvas_editor::editing::command::{
    Command, DuplicateLinesCommand, MoveLinesCommand, ToggleCommentCommand,
    line_comment_token,
};
use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Returns the inclusive line range affected by the primary cursor.
    ///
    /// When the primary cursor has a selection, the range covers every line it
    /// spans. A selection that ends at column 0 of a line does not include that
    /// trailing line (VS Code convention). Without a selection, the range is the
    /// single line the cursor sits on.
    fn primary_line_range(&self) -> (usize, usize) {
        let primary = self.cursors.primary();
        match primary.selection_range() {
            Some((sel_start, sel_end)) => {
                let end_line = if sel_end.1 == 0 && sel_end.0 > sel_start.0 {
                    sel_end.0 - 1
                } else {
                    sel_end.0
                };
                (sel_start.0, end_line)
            }
            None => {
                let line = primary.position.0;
                (line, line)
            }
        }
    }

    /// Shifts the primary cursor's position and selection anchor by `delta`
    /// whole lines (positive moves downward) so the selection follows an edit.
    fn shift_primary_cursor_lines(&mut self, delta: isize) {
        let primary = self.cursors.primary_mut();
        primary.position.0 = primary.position.0.saturating_add_signed(delta);
        if let Some(anchor) = primary.anchor.as_mut() {
            anchor.0 = anchor.0.saturating_add_signed(delta);
        }
    }

    /// Moves the current line, or the lines spanned by the primary selection,
    /// up or down by one line (Alt+Up / Alt+Down).
    ///
    /// Secondary cursors are collapsed onto the primary one. The move is a no-op
    /// when the affected range is already at the corresponding edge of the
    /// buffer.
    ///
    /// # Arguments
    ///
    /// * `down` - `true` to move the range down, `false` to move it up
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn move_lines(&mut self, down: bool) -> Task<Message> {
        self.end_grouping_if_active();
        self.cursors.remove_all_but_primary();

        let (start, end) = self.primary_line_range();

        // Reject moves that would push the range past the buffer edges.
        if down {
            if end + 1 >= self.buffer.line_count() {
                return Task::none();
            }
        } else if start == 0 {
            return Task::none();
        }

        let pos = self.cursors.primary_position();
        let mut cmd = MoveLinesCommand::new(start, end, down, pos);
        let mut cursor_pos = pos;
        cmd.execute(&mut self.buffer, &mut cursor_pos);
        self.shift_primary_cursor_lines(if down { 1 } else { -1 });
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    /// Duplicates the current line, or the lines spanned by the primary
    /// selection, above or below (Shift+Alt+Up / Shift+Alt+Down).
    ///
    /// Secondary cursors are collapsed onto the primary one. A downward
    /// duplication moves the cursor onto the new copy; an upward one leaves it
    /// on the (upper) copy.
    ///
    /// # Arguments
    ///
    /// * `down` - `true` to insert the copy below, `false` to insert it above
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn duplicate_lines(&mut self, down: bool) -> Task<Message> {
        self.end_grouping_if_active();
        self.cursors.remove_all_but_primary();

        let (start, end) = self.primary_line_range();
        let pos = self.cursors.primary_position();
        let mut cmd = DuplicateLinesCommand::new(start, end, down, pos);
        let mut cursor_pos = pos;
        cmd.execute(&mut self.buffer, &mut cursor_pos);
        if down {
            let block_len = (end - start + 1) as isize;
            self.shift_primary_cursor_lines(block_len);
        }
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    /// Toggles line comments on the current line, or the lines spanned by the
    /// primary selection (Ctrl+/).
    ///
    /// Secondary cursors are collapsed onto the primary one. If every non-blank
    /// line in the range is already commented, the range is uncommented;
    /// otherwise every non-blank line is commented. The operation is a no-op
    /// when the active syntax has no line-comment token (e.g. HTML) or the range
    /// holds only blank lines.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn toggle_comment(&mut self) -> Task<Message> {
        self.end_grouping_if_active();
        self.cursors.remove_all_but_primary();

        let Some(token) = line_comment_token(&self.syntax) else {
            return Task::none();
        };

        let (start, end) = self.primary_line_range();
        let pos = self.cursors.primary_position();
        let mut cmd =
            ToggleCommentCommand::new(&self.buffer, start, end, token, pos);
        if cmd.is_noop() {
            return Task::none();
        }

        // Track the selection anchor across the column shift before executing.
        let new_anchor =
            self.cursors.primary().anchor.map(|a| cmd.adjust_position(a));

        let mut cursor_pos = pos;
        cmd.execute(&mut self.buffer, &mut cursor_pos);
        let primary = self.cursors.primary_mut();
        primary.position = cursor_pos;
        primary.anchor = new_anchor;
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toggle_comment_selection() {
        let mut editor = CodeEditor::new("a\nb\nc", "rs");
        // Select lines 0..=2.
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (2, 1);

        let _ = editor.update(&Message::ToggleComment);
        assert_eq!(editor.buffer.to_string(), "// a\n// b\n// c");
        assert_eq!(editor.cursors.primary_position(), (2, 4));

        // Toggling again uncomments the whole range.
        let _ = editor.update(&Message::ToggleComment);
        assert_eq!(editor.buffer.to_string(), "a\nb\nc");
    }

    #[test]
    fn test_toggle_comment_noop_without_token() {
        let mut editor = CodeEditor::new("<div>", "html");
        let _ = editor.update(&Message::ToggleComment);
        // HTML has no line-comment token, so the buffer is unchanged.
        assert_eq!(editor.buffer.line(0), "<div>");
    }

    #[test]
    fn test_toggle_comment_undo() {
        let mut editor = CodeEditor::new("    let x = 1;", "rs");
        editor.cursors.primary_mut().position = (0, 8);

        let _ = editor.update(&Message::ToggleComment);
        assert_eq!(editor.buffer.line(0), "    // let x = 1;");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "    let x = 1;");
        assert_eq!(editor.cursors.primary_position(), (0, 8));
    }

    // =========================================================================
    // Line move / duplicate
    // =========================================================================
    //
    // `MoveLinesCommand` and `DuplicateLinesCommand` are well covered in
    // `editing/command/lines.rs`; what the tests below pin is the handler
    // logic layered on top of them, none of which lives in the commands: the
    // buffer-edge rejection, the cursor/anchor shift that keeps a selection
    // attached to the lines it follows, and `primary_line_range`'s VS Code
    // convention for a selection ending at column 0.

    /// Places the primary cursor at `position` with no selection.
    fn cursor_at(editor: &mut CodeEditor, position: (usize, usize)) {
        editor.cursors.primary_mut().anchor = None;
        editor.cursors.primary_mut().position = position;
    }

    /// Selects from `anchor` to `position` with the primary cursor.
    fn select(
        editor: &mut CodeEditor,
        anchor: (usize, usize),
        position: (usize, usize),
    ) {
        editor.cursors.primary_mut().anchor = Some(anchor);
        editor.cursors.primary_mut().position = position;
    }

    /// Returns the primary cursor's selection anchor, if any.
    fn anchor_of(editor: &CodeEditor) -> Option<(usize, usize)> {
        editor.cursors.primary().anchor
    }

    #[test]
    fn test_move_line_down_swaps_with_the_line_below() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        cursor_at(&mut editor, (0, 1));

        let _ = editor.update(&Message::MoveLineDown);

        assert_eq!(editor.buffer.to_string(), "two\none\nthree");
        // The cursor follows the line it was on, keeping its column.
        assert_eq!(editor.cursors.primary_position(), (1, 1));
    }

    #[test]
    fn test_move_line_up_swaps_with_the_line_above() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        cursor_at(&mut editor, (2, 2));

        let _ = editor.update(&Message::MoveLineUp);

        assert_eq!(editor.buffer.to_string(), "one\nthree\ntwo");
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_move_line_is_undoable() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        cursor_at(&mut editor, (0, 1));

        let _ = editor.update(&Message::MoveLineDown);
        assert_eq!(editor.buffer.to_string(), "two\none\nthree");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "one\ntwo\nthree");
        assert_eq!(editor.cursors.primary_position(), (0, 1));
    }

    #[test]
    fn test_move_line_at_the_buffer_edge_is_a_no_op() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");

        // Up from the first line has nowhere to go.
        cursor_at(&mut editor, (0, 0));
        let _ = editor.update(&Message::MoveLineUp);
        assert_eq!(editor.buffer.to_string(), "one\ntwo");

        // Nor down from the last line.
        cursor_at(&mut editor, (1, 0));
        let _ = editor.update(&Message::MoveLineDown);
        assert_eq!(editor.buffer.to_string(), "one\ntwo");

        // The rejection happens before the command is built, so no empty
        // entry lands on the undo stack for the user to step through.
        assert!(!editor.can_undo());
    }

    #[test]
    fn test_move_lines_carries_a_multi_line_selection_along() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        // Lines 0..=1 selected, cursor at the end of "two".
        select(&mut editor, (0, 0), (1, 3));

        let _ = editor.update(&Message::MoveLineDown);

        assert_eq!(editor.buffer.to_string(), "three\none\ntwo");
        // Both ends of the selection shift with the block, so the same text
        // stays selected afterwards.
        assert_eq!(editor.cursors.primary_position(), (2, 3));
        assert_eq!(anchor_of(&editor), Some((1, 0)));
    }

    #[test]
    fn test_a_selection_ending_at_column_zero_excludes_that_line() {
        let mut editor = CodeEditor::new("one\ntwo\nthree\nfour", "txt");
        // Selection stops at the very start of line 2, which by the VS Code
        // convention means lines 0..=1 are affected, not 0..=2.
        select(&mut editor, (0, 0), (2, 0));

        let _ = editor.update(&Message::MoveLineDown);

        // "three" — the line just past the range — is what moves up. Had line
        // 2 been included, "four" would have moved instead.
        assert_eq!(editor.buffer.to_string(), "three\none\ntwo\nfour");
    }

    #[test]
    fn test_duplicate_line_down_puts_the_cursor_on_the_copy() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        cursor_at(&mut editor, (0, 2));

        let _ = editor.update(&Message::DuplicateLineDown);

        assert_eq!(editor.buffer.to_string(), "one\none\ntwo");
        // Downward duplication moves the cursor onto the new copy below.
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_duplicate_line_up_leaves_the_cursor_on_the_upper_copy() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        cursor_at(&mut editor, (1, 2));

        let _ = editor.update(&Message::DuplicateLineUp);

        assert_eq!(editor.buffer.to_string(), "one\ntwo\ntwo");
        // The copy is inserted above, so the original index now names the
        // copy and the cursor does not move.
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_duplicate_lines_down_shifts_the_cursor_by_the_whole_block() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        select(&mut editor, (0, 0), (1, 3));

        let _ = editor.update(&Message::DuplicateLineDown);

        assert_eq!(editor.buffer.to_string(), "one\ntwo\none\ntwo\nthree");
        // Shifted by the block length (2), not by one line.
        assert_eq!(editor.cursors.primary_position(), (3, 3));
        assert_eq!(anchor_of(&editor), Some((2, 0)));
    }

    #[test]
    fn test_duplicate_line_works_at_the_buffer_edges() {
        // Unlike moving, duplicating is always legal: there is no neighbour
        // to swap with, so neither edge is rejected.
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        cursor_at(&mut editor, (1, 0));
        let _ = editor.update(&Message::DuplicateLineDown);
        assert_eq!(editor.buffer.to_string(), "one\ntwo\ntwo");

        let mut editor = CodeEditor::new("one\ntwo", "txt");
        cursor_at(&mut editor, (0, 0));
        let _ = editor.update(&Message::DuplicateLineUp);
        assert_eq!(editor.buffer.to_string(), "one\none\ntwo");
    }

    #[test]
    fn test_duplicate_lines_is_one_undo_step_for_the_whole_block() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        select(&mut editor, (0, 0), (1, 3));

        let _ = editor.update(&Message::DuplicateLineDown);
        assert_eq!(editor.buffer.to_string(), "one\ntwo\none\ntwo\nthree");
        assert_eq!(editor.history.undo_count(), 1);

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "one\ntwo\nthree");
    }

    #[test]
    fn test_line_operations_collapse_secondary_cursors() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        cursor_at(&mut editor, (0, 0));
        editor.cursors.add_cursor((2, 0));
        assert!(editor.cursors.is_multi());

        // These commands act on the primary cursor's range only, so they
        // collapse the extra cursors rather than silently ignoring them.
        let _ = editor.update(&Message::MoveLineDown);
        assert!(!editor.cursors.is_multi());

        editor.cursors.add_cursor((2, 0));
        let _ = editor.update(&Message::DuplicateLineDown);
        assert!(!editor.cursors.is_multi());
    }
}
