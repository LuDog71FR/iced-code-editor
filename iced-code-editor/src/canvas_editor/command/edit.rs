//! Single-buffer editing commands: character insertion/deletion, newline
//! insertion, and range operations.

use crate::text_buffer::TextBuffer;

use super::Command;

/// Inserts `text` into `buffer` starting at `(line, col)`, character by
/// character, translating `'\n'` into a newline split and every other
/// character into a plain insertion.
///
/// Returns the buffer position immediately after the last inserted
/// character (i.e. where the caret would rest after typing `text`).
///
/// # Examples
///
/// ```ignore
/// let end = insert_text_at(&mut buffer, 0, 0, "ab\ncd");
/// assert_eq!(end, (1, 2));
/// ```
fn insert_text_at(
    buffer: &mut TextBuffer,
    line: usize,
    col: usize,
    text: &str,
) -> (usize, usize) {
    let mut current_line = line;
    let mut current_col = col;

    for ch in text.chars() {
        if ch == '\n' {
            buffer.insert_newline(current_line, current_col);
            current_line += 1;
            current_col = 0;
        } else {
            buffer.insert_char(current_line, current_col, ch);
            current_col += 1;
        }
    }

    (current_line, current_col)
}

/// Command for inserting a single character.
#[derive(Debug, Clone)]
pub struct InsertCharCommand {
    line: usize,
    col: usize,
    ch: char,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
}

impl InsertCharCommand {
    /// Creates a new insert character command.
    ///
    /// # Arguments
    ///
    /// * `line` - Line index where to insert
    /// * `col` - Column position where to insert
    /// * `ch` - Character to insert
    /// * `cursor` - Current cursor position
    pub fn new(
        line: usize,
        col: usize,
        ch: char,
        cursor: (usize, usize),
    ) -> Self {
        Self {
            line,
            col,
            ch,
            cursor_before: cursor,
            cursor_after: (line, col + 1),
        }
    }
}

impl Command for InsertCharCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        buffer.insert_char(self.line, self.col, self.ch);
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        // Delete the character we inserted
        buffer.delete_forward(self.line, self.col);
        *cursor = self.cursor_before;
    }
}

/// Command for deleting a character (backspace).
#[derive(Debug, Clone)]
pub struct DeleteCharCommand {
    line: usize,
    col: usize,
    deleted_char: Option<char>,
    merged_line: bool,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
}

impl DeleteCharCommand {
    /// Creates a new delete character command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer (to read the character being deleted)
    /// * `line` - Line index
    /// * `col` - Column position
    /// * `cursor` - Current cursor position
    pub fn new(
        buffer: &TextBuffer,
        line: usize,
        col: usize,
        cursor: (usize, usize),
    ) -> Self {
        let (deleted_char, merged_line, cursor_after) = if col > 0 {
            // Deleting character before cursor
            let line_str = buffer.line(line);
            let ch = line_str.chars().nth(col - 1);
            (ch, false, (line, col - 1))
        } else if line > 0 {
            // Merging with previous line
            let prev_line_len = buffer.line_len(line - 1);
            (None, true, (line - 1, prev_line_len))
        } else {
            // At beginning of document, nothing to delete
            (None, false, cursor)
        };

        Self {
            line,
            col,
            deleted_char,
            merged_line,
            cursor_before: cursor,
            cursor_after,
        }
    }
}

impl Command for DeleteCharCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        buffer.delete_char(self.line, self.col);
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        if self.merged_line {
            // Splitting the merged line back at the join point is enough: the
            // text after the split is exactly the line that was merged in.
            // Re-inserting it on top would duplicate it.
            buffer.insert_newline(self.cursor_after.0, self.cursor_after.1);
        } else if let Some(ch) = self.deleted_char {
            // Re-insert the deleted character
            buffer.insert_char(self.line, self.col - 1, ch);
        }
        *cursor = self.cursor_before;
    }
}

/// Command for deleting forward (Delete key).
#[derive(Debug, Clone)]
pub struct DeleteForwardCommand {
    line: usize,
    col: usize,
    deleted_char: Option<char>,
    merged_next_line: bool,
    cursor_before: (usize, usize),
}

impl DeleteForwardCommand {
    /// Creates a new delete forward command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer
    /// * `line` - Line index
    /// * `col` - Column position
    /// * `cursor` - Current cursor position
    pub fn new(
        buffer: &TextBuffer,
        line: usize,
        col: usize,
        cursor: (usize, usize),
    ) -> Self {
        let line_len = buffer.line_len(line);
        let (deleted_char, merged_next_line) = if col < line_len {
            // Deleting character at cursor
            let ch = buffer.line(line).chars().nth(col);
            (ch, false)
        } else if line + 1 < buffer.line_count() {
            // Merging with next line
            (None, true)
        } else {
            // At end of document
            (None, false)
        };

        Self {
            line,
            col,
            deleted_char,
            merged_next_line,
            cursor_before: cursor,
        }
    }
}

impl Command for DeleteForwardCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        buffer.delete_forward(self.line, self.col);
        *cursor = self.cursor_before; // Cursor doesn't move on delete forward
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        if self.merged_next_line {
            // Splitting the merged line back at the join point is enough: the
            // text after the split is exactly the line that was merged in.
            // Re-inserting it on top would duplicate it.
            buffer.insert_newline(self.line, self.col);
        } else if let Some(ch) = self.deleted_char {
            // Re-insert the deleted character
            buffer.insert_char(self.line, self.col, ch);
        }
        *cursor = self.cursor_before;
    }
}

/// Command for inserting a newline, optionally with auto-indentation.
///
/// When `indent` is non-empty, the indentation string is inserted at the
/// beginning of the new line after the split, and the cursor is placed
/// after the indent. Undo removes the indent chars then joins the lines.
#[derive(Debug, Clone)]
pub struct InsertNewlineCommand {
    line: usize,
    col: usize,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
    indent: String,
}

impl InsertNewlineCommand {
    /// Creates a new insert newline command with auto-indentation.
    ///
    /// The `indent` string (leading whitespace of the current line) is
    /// inserted at the start of the new line, and the cursor is placed
    /// after it.
    ///
    /// # Arguments
    ///
    /// * `line` - Line index where to insert
    /// * `col` - Column position where to split
    /// * `cursor` - Current cursor position
    /// * `indent` - Leading whitespace to copy to the new line
    pub fn with_indent(
        line: usize,
        col: usize,
        cursor: (usize, usize),
        indent: String,
    ) -> Self {
        let indent_len = indent.chars().count();
        Self {
            line,
            col,
            cursor_before: cursor,
            cursor_after: (line + 1, indent_len),
            indent,
        }
    }
}

impl Command for InsertNewlineCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        buffer.insert_newline(self.line, self.col);
        for (i, c) in self.indent.chars().enumerate() {
            buffer.insert_char(self.line + 1, i, c);
        }
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        // Remove indent chars inserted at start of new line
        for _ in 0..self.indent.chars().count() {
            buffer.delete_forward(self.line + 1, 0);
        }
        // Merge the two lines back together
        if self.line + 1 < buffer.line_count() {
            buffer.delete_char(self.line + 1, 0);
        }
        *cursor = self.cursor_before;
    }
}

/// Command for inserting multiple characters (paste).
#[derive(Debug, Clone)]
pub struct InsertTextCommand {
    line: usize,
    col: usize,
    text: String,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
    /// Position immediately after the last inserted character.
    ///
    /// This is deliberately separate from `cursor_after`, which callers may
    /// override to rest the caret somewhere else (Vim paste leaves it on the
    /// first pasted character). [`Command::undo`] deletes the inserted text by
    /// walking backwards from this position, so it must always describe the end
    /// of the insertion, never where the caret happens to end up.
    insert_end: (usize, usize),
}

impl InsertTextCommand {
    /// Creates a new insert text command.
    ///
    /// # Arguments
    ///
    /// * `line` - Line index where to insert
    /// * `col` - Column position where to insert
    /// * `text` - Text to insert
    /// * `cursor` - Current cursor position
    pub fn new(
        line: usize,
        col: usize,
        text: String,
        cursor: (usize, usize),
    ) -> Self {
        // Position right after the inserted text; also the default resting
        // place for the cursor.
        let lines: Vec<&str> = text.split('\n').collect();
        let insert_end = if lines.len() == 1 {
            (line, col + text.chars().count())
        } else {
            let last_line_len = lines.last().map_or(0, |l| l.chars().count());
            (line + lines.len() - 1, last_line_len)
        };

        Self {
            line,
            col,
            text,
            cursor_before: cursor,
            cursor_after: insert_end,
            insert_end,
        }
    }

    /// Overrides the cursor position restored when this insertion is redone.
    ///
    /// Most paste operations leave the cursor after the inserted text, while
    /// Vim paste leaves it on the first inserted character or line. Only the
    /// resting position changes; `insert_end` keeps describing the end of the
    /// insertion so undo still removes exactly the inserted characters.
    pub(crate) fn with_cursor_after(
        mut self,
        cursor_after: (usize, usize),
    ) -> Self {
        self.cursor_after = cursor_after;
        self
    }
}

impl Command for InsertTextCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        insert_text_at(buffer, self.line, self.col, &self.text);
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        // Delete characters in reverse, starting from the end of the inserted
        // text rather than from wherever the caret was left to rest.
        let mut current_line = self.insert_end.0;
        let mut current_col = self.insert_end.1;

        for ch in self.text.chars().rev() {
            if ch == '\n' {
                // Merge lines
                if current_line > 0 {
                    let prev_line_len = buffer.line_len(current_line - 1);
                    buffer.delete_char(current_line, 0);
                    current_line -= 1;
                    current_col = prev_line_len;
                }
            } else {
                // Delete character
                if current_col > 0 {
                    buffer.delete_char(current_line, current_col);
                    current_col -= 1;
                }
            }
        }

        *cursor = self.cursor_before;
    }
}

/// Command for deleting a range of text (selection).
#[derive(Debug, Clone)]
pub struct DeleteRangeCommand {
    start: (usize, usize),
    end: (usize, usize),
    deleted_text: String,
    cursor_before: (usize, usize),
}

impl DeleteRangeCommand {
    /// Creates a new delete range command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer
    /// * `start` - Start position (line, col)
    /// * `end` - End position (line, col)
    /// * `cursor` - Current cursor position
    pub fn new(
        buffer: &TextBuffer,
        start: (usize, usize),
        end: (usize, usize),
        cursor: (usize, usize),
    ) -> Self {
        // Extract the text being deleted
        let mut deleted_text = String::new();

        if start.0 == end.0 {
            // Single line
            let line = buffer.line(start.0);
            let chars: Vec<char> = line.chars().collect();
            for ch in chars.iter().skip(start.1).take(
                end.1
                    .saturating_sub(start.1)
                    .min(chars.len().saturating_sub(start.1)),
            ) {
                deleted_text.push(*ch);
            }
        } else {
            // Multiple lines
            for line_idx in start.0..=end.0 {
                let line = buffer.line(line_idx);
                let chars: Vec<char> = line.chars().collect();

                if line_idx == start.0 {
                    // First line: from start.1 to end
                    for ch in chars.iter().skip(start.1) {
                        deleted_text.push(*ch);
                    }
                    deleted_text.push('\n');
                } else if line_idx == end.0 {
                    // Last line: from 0 to end.1
                    for ch in chars.iter().take(end.1.min(chars.len())) {
                        deleted_text.push(*ch);
                    }
                } else {
                    // Middle lines: entire line
                    deleted_text.push_str(line);
                    deleted_text.push('\n');
                }
            }
        }

        Self { start, end, deleted_text, cursor_before: cursor }
    }
}

impl Command for DeleteRangeCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        // Delete from start to end
        if self.start == self.end {
            *cursor = self.start;
            return;
        }

        if self.start.0 == self.end.0 {
            // Single line: remove the selected characters in one splice.
            buffer.replace_range(
                self.start.0,
                self.start.1,
                self.end.1 - self.start.1,
                "",
            );
        } else {
            // Multi-line: splice the surviving tail of the last line onto
            // the first line's prefix, then drop the fully-consumed lines
            // in between (including the original last line). This keeps
            // the whole operation O(text touched + lines removed) instead
            // of the previous per-character `delete_forward` loop.
            let tail: String =
                buffer.line(self.end.0).chars().skip(self.end.1).collect();
            let first_line_len = buffer.line_len(self.start.0);
            buffer.replace_range(
                self.start.0,
                self.start.1,
                first_line_len - self.start.1,
                &tail,
            );
            for _ in self.start.0..self.end.0 {
                buffer.remove_line(self.start.0 + 1);
            }
        }

        *cursor = self.start;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        // Re-insert the deleted text by reversing the splice performed in
        // `execute`, using the exact captured text instead of replaying it
        // character by character.
        if self.start.0 == self.end.0 {
            buffer.replace_range(
                self.start.0,
                self.start.1,
                0,
                &self.deleted_text,
            );
        } else {
            let segments: Vec<&str> = self.deleted_text.split('\n').collect();
            if let [first_segment, middle_segments @ .., last_segment] =
                segments.as_slice()
            {
                let tail: String = buffer
                    .line(self.start.0)
                    .chars()
                    .skip(self.start.1)
                    .collect();
                buffer.replace_range(
                    self.start.0,
                    self.start.1,
                    tail.chars().count(),
                    first_segment,
                );
                for (offset, segment) in middle_segments.iter().enumerate() {
                    buffer.insert_line(
                        self.start.0 + 1 + offset,
                        (*segment).to_string(),
                    );
                }
                buffer.insert_line(self.end.0, format!("{last_segment}{tail}"));
            }
        }
        *cursor = self.cursor_before;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_text_at_single_line() {
        let mut buffer = TextBuffer::new("hello");
        let end = insert_text_at(&mut buffer, 0, 5, "!!");
        assert_eq!(buffer.line(0), "hello!!");
        assert_eq!(end, (0, 7));
    }

    #[test]
    fn test_insert_text_at_with_newlines_returns_end_position() {
        let mut buffer = TextBuffer::new("ac");
        let end = insert_text_at(&mut buffer, 0, 1, "b\nb");
        assert_eq!(buffer.line(0), "ab");
        assert_eq!(buffer.line(1), "bc");
        assert_eq!(end, (1, 1));
    }

    #[test]
    fn test_insert_text_at_empty_string_is_noop() {
        let mut buffer = TextBuffer::new("hello");
        let end = insert_text_at(&mut buffer, 0, 2, "");
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(end, (0, 2));
    }

    #[test]
    fn test_insert_char_command() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let mut cmd = InsertCharCommand::new(0, 5, '!', cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello!");
        assert_eq!(cursor, (0, 6));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_delete_char_command() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let mut cmd = DeleteCharCommand::new(&buffer, 0, 5, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hell");
        assert_eq!(cursor, (0, 4));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_delete_char_command_merge_undo_restores_both_lines() {
        // Backspace at column 0 merges the line into the previous one; undo
        // must split it back without duplicating the merged content.
        let mut buffer = TextBuffer::new("hello\nworld");
        let mut cursor = (1, 0);
        let mut cmd = DeleteCharCommand::new(&buffer, 1, 0, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "helloworld");
        assert_eq!(cursor, (0, 5));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello\nworld");
        assert_eq!(cursor, (1, 0));
    }

    #[test]
    fn test_delete_char_command_merge_undo_with_empty_lines() {
        // Merging an empty line into a non-empty one, and vice versa.
        // `"text\n\n"` parses to two lines ("text", ""): `str::lines()` does
        // not emit a trailing empty line for a single trailing `\n`. Backspace
        // at (1, 0) deletes exactly the newline separating them, so the
        // result keeps the second `\n` (the one terminating the now-merged
        // line) rather than dropping both.
        let mut buffer = TextBuffer::new("text\n\n");
        let mut cursor = (1, 0);
        let mut cmd = DeleteCharCommand::new(&buffer, 1, 0, cursor);
        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "text\n");
        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "text\n\n");

        let mut buffer = TextBuffer::new("\ntext");
        let mut cursor = (1, 0);
        let mut cmd = DeleteCharCommand::new(&buffer, 1, 0, cursor);
        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "text");
        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "\ntext");
    }

    #[test]
    fn test_delete_forward_command() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 4);
        let mut cmd = DeleteForwardCommand::new(&buffer, 0, 4, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hell");
        assert_eq!(cursor, (0, 4));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(cursor, (0, 4));
    }

    #[test]
    fn test_delete_forward_command_merge_undo_restores_both_lines() {
        // Delete at end of line merges the next line into it; undo must
        // split it back without duplicating the merged content.
        let mut buffer = TextBuffer::new("hello\nworld");
        let mut cursor = (0, 5);
        let mut cmd = DeleteForwardCommand::new(&buffer, 0, 5, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "helloworld");
        assert_eq!(cursor, (0, 5));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello\nworld");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_delete_forward_command_merge_undo_with_empty_lines() {
        // Merging an empty line into a non-empty one, and vice versa.
        // `"text\n\n"` parses to two lines ("text", ""): `str::lines()` does
        // not emit a trailing empty line for a single trailing `\n`. Forward
        // delete at (0, 4) deletes exactly the newline separating them, so
        // the result keeps the second `\n` (the one terminating the
        // now-merged line) rather than dropping both.
        let mut buffer = TextBuffer::new("text\n\n");
        let mut cursor = (0, 4);
        let mut cmd = DeleteForwardCommand::new(&buffer, 0, 4, cursor);
        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "text\n");
        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "text\n\n");

        let mut buffer = TextBuffer::new("\ntext");
        let mut cursor = (0, 0);
        let mut cmd = DeleteForwardCommand::new(&buffer, 0, 0, cursor);
        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "text");
        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "\ntext");
    }

    #[test]
    fn test_insert_newline_command() {
        let mut buffer = TextBuffer::new("hello world");
        let mut cursor = (0, 5);
        let mut cmd =
            InsertNewlineCommand::with_indent(0, 5, cursor, String::new());

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(buffer.line(1), " world");
        assert_eq!(cursor, (1, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello world");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_insert_newline_with_indent_spaces() {
        let mut buffer = TextBuffer::new("    hello");
        let mut cursor = (0, 9);
        let mut cmd =
            InsertNewlineCommand::with_indent(0, 9, cursor, "    ".to_string());

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "    hello");
        assert_eq!(buffer.line(1), "    ");
        assert_eq!(cursor, (1, 4));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "    hello");
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(cursor, (0, 9));
    }

    #[test]
    fn test_insert_newline_with_indent_mid_line() {
        let mut buffer = TextBuffer::new("    hello world");
        let mut cursor = (0, 9);
        let mut cmd =
            InsertNewlineCommand::with_indent(0, 9, cursor, "    ".to_string());

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "    hello");
        assert_eq!(buffer.line(1), "     world"); // 4 spaces indent + " world"
        assert_eq!(cursor, (1, 4));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "    hello world");
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(cursor, (0, 9));
    }

    #[test]
    fn test_insert_newline_with_indent_tab() {
        let mut buffer = TextBuffer::new("\thello");
        let mut cursor = (0, 6);
        let mut cmd =
            InsertNewlineCommand::with_indent(0, 6, cursor, "\t".to_string());

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "\thello");
        assert_eq!(buffer.line(1), "\t");
        assert_eq!(cursor, (1, 1));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "\thello");
        assert_eq!(buffer.line_count(), 1);
        assert_eq!(cursor, (0, 6));
    }

    #[test]
    fn test_insert_text_command() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let mut cmd =
            InsertTextCommand::new(0, 5, " world".to_string(), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello world");
        assert_eq!(cursor, (0, 11));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_insert_text_command_undo_ignores_overridden_cursor_after() {
        // Vim paste rests the caret on the first pasted character. Undo must
        // still delete the inserted text, which starts at the caret.
        let mut buffer = TextBuffer::new("abc");
        let mut cursor = (0, 0);
        let mut cmd = InsertTextCommand::new(0, 0, "a".to_string(), cursor)
            .with_cursor_after((0, 0));

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "aabc");
        assert_eq!(cursor, (0, 0), "caret rests on the pasted character");

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "abc");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_insert_text_command_undo_multiline_with_overridden_cursor_after() {
        // Linewise Vim paste: the caret rests at the start of the pasted line.
        let mut buffer = TextBuffer::new("one\ntwo");
        let mut cursor = (0, 0);
        let mut cmd = InsertTextCommand::new(1, 0, "one\n".to_string(), cursor)
            .with_cursor_after((1, 0));

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "one\none\ntwo");
        assert_eq!(cursor, (1, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "one\ntwo");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_delete_range_command() {
        let mut buffer = TextBuffer::new("hello world");
        let mut cursor = (0, 0);
        let mut cmd = DeleteRangeCommand::new(&buffer, (0, 0), (0, 5), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), " world");
        assert_eq!(cursor, (0, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello world");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_delete_range_command_multiline() {
        let mut buffer = TextBuffer::new("hello\nworld\nfoo\nbar");
        let mut cursor = (0, 0);
        let mut cmd = DeleteRangeCommand::new(&buffer, (0, 2), (2, 1), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "heoo\nbar");
        assert_eq!(cursor, (0, 2));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello\nworld\nfoo\nbar");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_delete_range_command_range_ending_at_column_zero() {
        let mut buffer = TextBuffer::new("ab\ncd");
        let mut cursor = (0, 0);
        let mut cmd = DeleteRangeCommand::new(&buffer, (0, 1), (1, 0), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "acd");
        assert_eq!(cursor, (0, 1));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "ab\ncd");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_delete_range_command_empty_selection() {
        let mut buffer = TextBuffer::new("hello world");
        let mut cursor = (0, 3);
        let mut cmd = DeleteRangeCommand::new(&buffer, (0, 3), (0, 3), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello world");
        assert_eq!(cursor, (0, 3));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello world");
        assert_eq!(cursor, (0, 3));
    }

    #[test]
    fn test_delete_range_command_at_buffer_start() {
        let mut buffer = TextBuffer::new("hello\nworld");
        let mut cursor = (0, 0);
        let mut cmd = DeleteRangeCommand::new(&buffer, (0, 0), (1, 2), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "rld");
        assert_eq!(cursor, (0, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello\nworld");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_delete_range_command_at_buffer_end() {
        let mut buffer = TextBuffer::new("hello\nworld");
        let mut cursor = (0, 0);
        let mut cmd = DeleteRangeCommand::new(&buffer, (0, 3), (1, 5), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hel");
        assert_eq!(cursor, (0, 3));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "hello\nworld");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_delete_range_command_many_lines() {
        let original = "one\ntwo\nthree\nfour\nfive\nsix\nseven";
        let mut buffer = TextBuffer::new(original);
        let mut cursor = (0, 0);
        let mut cmd = DeleteRangeCommand::new(&buffer, (1, 1), (5, 2), cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "one\ntx\nseven");
        assert_eq!(cursor, (1, 1));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), original);
        assert_eq!(cursor, (0, 0));
    }
}
