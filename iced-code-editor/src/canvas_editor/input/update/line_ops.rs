//! Line move/duplicate and comment-toggle message handlers.

use iced::Task;

use crate::canvas_editor::command::{
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
}
