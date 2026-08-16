//! Line-level commands: moving and duplicating contiguous ranges of lines.

use crate::buffer::TextBuffer;

use super::Command;

/// Command for moving a contiguous range of lines up or down by one line.
///
/// The range `[start, end]` (inclusive) is swapped with the adjacent line
/// above (`down = false`) or below (`down = true`). Callers must ensure the
/// move is legal: `start > 0` for an upward move and `end + 1 < line_count`
/// for a downward move.
#[derive(Debug, Clone)]
pub struct MoveLinesCommand {
    start: usize,
    end: usize,
    down: bool,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
}

impl MoveLinesCommand {
    /// Creates a new move-lines command.
    ///
    /// # Arguments
    ///
    /// * `start` - First line of the range to move (inclusive)
    /// * `end` - Last line of the range to move (inclusive)
    /// * `down` - `true` to move the range down, `false` to move it up
    /// * `cursor` - Current cursor position
    pub fn new(
        start: usize,
        end: usize,
        down: bool,
        cursor: (usize, usize),
    ) -> Self {
        let cursor_after = if down {
            (cursor.0 + 1, cursor.1)
        } else {
            (cursor.0 - 1, cursor.1)
        };
        Self { start, end, down, cursor_before: cursor, cursor_after }
    }
}

impl Command for MoveLinesCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        if self.down {
            // Pull the line below the range up to the top of the range.
            if let Some(line) = buffer.remove_line(self.end + 1) {
                buffer.insert_line(self.start, line);
            }
        } else {
            // Push the line above the range down to the bottom of the range.
            if let Some(line) = buffer.remove_line(self.start - 1) {
                buffer.insert_line(self.end, line);
            }
        }
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        if self.down {
            // The moved line is now at `start`; send it back below the range.
            if let Some(line) = buffer.remove_line(self.start) {
                buffer.insert_line(self.end + 1, line);
            }
        } else {
            // The moved line is now at `end`; send it back above the range.
            if let Some(line) = buffer.remove_line(self.end) {
                buffer.insert_line(self.start - 1, line);
            }
        }
        *cursor = self.cursor_before;
    }
}

/// Command for duplicating a contiguous range of lines.
///
/// A copy of the range `[start, end]` (inclusive) is inserted directly below
/// the range (`down = true`) or directly above it (`down = false`).
#[derive(Debug, Clone)]
pub struct DuplicateLinesCommand {
    start: usize,
    end: usize,
    down: bool,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
}

impl DuplicateLinesCommand {
    /// Creates a new duplicate-lines command.
    ///
    /// # Arguments
    ///
    /// * `start` - First line of the range to duplicate (inclusive)
    /// * `end` - Last line of the range to duplicate (inclusive)
    /// * `down` - `true` to insert the copy below, `false` to insert it above
    /// * `cursor` - Current cursor position
    pub fn new(
        start: usize,
        end: usize,
        down: bool,
        cursor: (usize, usize),
    ) -> Self {
        let block_len = end - start + 1;
        // Downward: move the cursor onto the new copy below. Upward: the copy
        // is inserted above, so the original line index now points to the copy.
        let cursor_after =
            if down { (cursor.0 + block_len, cursor.1) } else { cursor };
        Self { start, end, down, cursor_before: cursor, cursor_after }
    }
}

impl Command for DuplicateLinesCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        let block: Vec<String> = (self.start..=self.end)
            .map(|i| buffer.line(i).to_string())
            .collect();
        let insert_at = if self.down { self.end + 1 } else { self.start };
        for (offset, content) in block.into_iter().enumerate() {
            buffer.insert_line(insert_at + offset, content);
        }
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        let block_len = self.end - self.start + 1;
        let remove_at = if self.down { self.end + 1 } else { self.start };
        for _ in 0..block_len {
            buffer.remove_line(remove_at);
        }
        *cursor = self.cursor_before;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_lines_command_down() {
        let mut buffer = TextBuffer::new("a\nb\nc");
        let mut cursor = (1, 0);
        let mut cmd = MoveLinesCommand::new(1, 1, true, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nc\nb");
        assert_eq!(cursor, (2, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc");
        assert_eq!(cursor, (1, 0));
    }

    #[test]
    fn test_move_lines_command_up() {
        let mut buffer = TextBuffer::new("a\nb\nc");
        let mut cursor = (2, 0);
        let mut cmd = MoveLinesCommand::new(2, 2, false, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nc\nb");
        assert_eq!(cursor, (1, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc");
        assert_eq!(cursor, (2, 0));
    }

    #[test]
    fn test_move_lines_command_range_down() {
        let mut buffer = TextBuffer::new("a\nb\nc\nd");
        let mut cursor = (1, 0);
        // Move the block [1, 2] (b, c) down.
        let mut cmd = MoveLinesCommand::new(1, 2, true, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nd\nb\nc");
        assert_eq!(cursor, (2, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc\nd");
        assert_eq!(cursor, (1, 0));
    }

    #[test]
    fn test_duplicate_lines_command_down() {
        let mut buffer = TextBuffer::new("a\nb\nc");
        let mut cursor = (1, 0);
        let mut cmd = DuplicateLinesCommand::new(1, 1, true, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nb\nc");
        assert_eq!(cursor, (2, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc");
        assert_eq!(cursor, (1, 0));
    }

    #[test]
    fn test_duplicate_lines_command_up() {
        let mut buffer = TextBuffer::new("a\nb\nc");
        let mut cursor = (1, 0);
        let mut cmd = DuplicateLinesCommand::new(1, 1, false, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nb\nc");
        // Cursor stays on the upper copy.
        assert_eq!(cursor, (1, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc");
        assert_eq!(cursor, (1, 0));
    }

    #[test]
    fn test_duplicate_lines_command_range_down() {
        let mut buffer = TextBuffer::new("a\nb\nc\nd");
        let mut cursor = (1, 0);
        // Duplicate the block [1, 2] (b, c) below.
        let mut cmd = DuplicateLinesCommand::new(1, 2, true, cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc\nb\nc\nd");
        assert_eq!(cursor, (3, 0));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "a\nb\nc\nd");
        assert_eq!(cursor, (1, 0));
    }
}
