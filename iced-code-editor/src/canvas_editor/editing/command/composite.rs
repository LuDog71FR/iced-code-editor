//! Composite and search/replace commands: grouping multiple commands into
//! one undo/redo step, and replacing a text range.

use crate::buffer::TextBuffer;

use super::Command;

/// Composite command that groups multiple commands together.
#[derive(Debug)]
pub struct CompositeCommand {
    commands: Vec<Box<dyn Command>>,
}

impl CompositeCommand {
    /// Creates a new, empty composite command.
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    /// Adds a command to this composite.
    pub fn add(&mut self, command: Box<dyn Command>) {
        self.commands.push(command);
    }

    /// Returns whether this composite is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Command for CompositeCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        for cmd in &mut self.commands {
            cmd.execute(buffer, cursor);
        }
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        // Undo in reverse order
        for cmd in self.commands.iter_mut().rev() {
            cmd.undo(buffer, cursor);
        }
    }
}

/// Command for replacing text (used in search/replace functionality).
#[derive(Debug, Clone)]
pub struct ReplaceTextCommand {
    position: (usize, usize),
    old_text: String,
    new_text: String,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
}

impl ReplaceTextCommand {
    /// Creates a new replace text command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer (to read the old text)
    /// * `position` - Start position (line, col) of text to replace
    /// * `old_text_len` - Length of text to replace (in characters)
    /// * `new_text` - Text to insert in place
    /// * `cursor` - Current cursor position
    pub fn new(
        buffer: &TextBuffer,
        position: (usize, usize),
        old_text_len: usize,
        new_text: String,
        cursor: (usize, usize),
    ) -> Self {
        let mut old_text = String::new();
        let mut remaining = old_text_len;
        for line in position.0..buffer.line_count() {
            let skip = if line == position.0 { position.1 } else { 0 };
            for ch in buffer.line(line).chars().skip(skip).take(remaining) {
                old_text.push(ch);
            }
            remaining = old_text_len.saturating_sub(old_text.chars().count());
            if remaining == 0 {
                break;
            }
            if line + 1 < buffer.line_count() {
                old_text.push('\n');
                remaining -= 1;
            }
            if remaining == 0 {
                break;
            }
        }
        let cursor_after = text_end(position, &new_text);

        Self {
            position,
            old_text,
            new_text,
            cursor_before: cursor,
            cursor_after,
        }
    }
}

impl Command for ReplaceTextCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        replace_text(buffer, self.position, &self.old_text, &self.new_text);

        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        replace_text(buffer, self.position, &self.new_text, &self.old_text);

        *cursor = self.cursor_before;
    }
}

fn text_end(start: (usize, usize), text: &str) -> (usize, usize) {
    text.chars().fold(start, |(line, col), ch| {
        if ch == '\n' { (line + 1, 0) } else { (line, col + 1) }
    })
}

fn replace_text(
    buffer: &mut TextBuffer,
    start: (usize, usize),
    old: &str,
    new: &str,
) {
    if !old.contains('\n') && !new.contains('\n') {
        buffer.replace_range(start.0, start.1, old.chars().count(), new);
        return;
    }
    for _ in old.chars() {
        buffer.delete_forward(start.0, start.1);
    }
    let (mut line, mut col) = start;
    for ch in new.chars() {
        if ch == '\n' {
            buffer.insert_newline(line, col);
            line += 1;
            col = 0;
        } else {
            buffer.insert_char(line, col, ch);
            col += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::edit::InsertCharCommand;
    use super::*;

    #[test]
    fn test_composite_command() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let mut composite = CompositeCommand::new();

        composite.add(Box::new(InsertCharCommand::new(0, 5, ' ', cursor)));
        cursor.1 += 1;
        composite.add(Box::new(InsertCharCommand::new(0, 6, 'w', cursor)));
        cursor.1 += 1;
        composite.add(Box::new(InsertCharCommand::new(0, 7, 'o', cursor)));

        composite.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello wo");

        composite.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
    }

    #[test]
    fn test_replace_text_command() {
        let mut buffer = TextBuffer::new("hello world");
        let mut cursor = (0, 0);
        let mut cmd = ReplaceTextCommand::new(
            &buffer,
            (0, 0),
            5,
            "goodbye".to_string(),
            cursor,
        );

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "goodbye world");
        assert_eq!(cursor, (0, 7));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello world");
        assert_eq!(cursor, (0, 0));
    }

    #[test]
    fn test_replace_text_different_lengths() {
        let mut buffer = TextBuffer::new("foo bar baz");
        let mut cursor = (0, 4);

        // Replace "bar" (3 chars) with "x" (1 char)
        let mut cmd = ReplaceTextCommand::new(
            &buffer,
            (0, 4),
            3,
            "x".to_string(),
            cursor,
        );

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "foo x baz");
        assert_eq!(cursor, (0, 5));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "foo bar baz");
        assert_eq!(cursor, (0, 4));
    }

    #[test]
    fn test_replace_all_composite() {
        let mut buffer = TextBuffer::new("foo foo foo");
        let mut cursor = (0, 0);
        let mut composite = CompositeCommand::new();

        // Replace all "foo" with "bar" (in reverse order to preserve positions)
        composite.add(Box::new(ReplaceTextCommand::new(
            &buffer,
            (0, 8),
            3,
            "bar".to_string(),
            cursor,
        )));
        composite.add(Box::new(ReplaceTextCommand::new(
            &buffer,
            (0, 4),
            3,
            "bar".to_string(),
            cursor,
        )));
        composite.add(Box::new(ReplaceTextCommand::new(
            &buffer,
            (0, 0),
            3,
            "bar".to_string(),
            cursor,
        )));

        composite.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "bar bar bar");

        composite.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "foo foo foo");
    }
}

#[cfg(test)]
mod multiline_tests {
    use super::*;

    #[test]
    fn multiline_replacements_restore_text_line_endings_and_cursor_on_undo_and_redo()
     {
        let cases = [
            ("前ab\n中cd\n尾", (0, 1), 5, "😀\nx", "前😀\nxd\n尾", (1, 1)),
            ("a\r\nb\r\n", (0, 1), 1, "", "ab\r\n", (0, 1)),
            ("a\nb", (0, 1), 0, "\n😀\n", "a\n😀\n\nb", (2, 0)),
            ("ab", (0, 1), 1, "x\ny\n", "ax\ny\n", (2, 0)),
        ];
        for (source, start, length, replacement, expected, end) in cases {
            let mut buffer = TextBuffer::new(source);
            let original = buffer.to_string();
            let cursor_before = (0, 0);
            let mut cursor = cursor_before;
            let mut command = ReplaceTextCommand::new(
                &buffer,
                start,
                length,
                replacement.into(),
                cursor,
            );
            command.execute(&mut buffer, &mut cursor);
            assert_eq!(buffer.to_string(), expected, "source={source:?}");
            assert_eq!(cursor, end);
            command.undo(&mut buffer, &mut cursor);
            assert_eq!(buffer.to_string(), original);
            assert_eq!(cursor, cursor_before);
            command.execute(&mut buffer, &mut cursor);
            assert_eq!(buffer.to_string(), expected);
            assert_eq!(cursor, end);
        }
    }
}
