//! Composite and search/replace commands: grouping multiple commands into
//! one undo/redo step, and replacing a text range.

use crate::text_buffer::TextBuffer;

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
        // Extract the old text being replaced
        let line = buffer.line(position.0);
        let chars: Vec<char> = line.chars().collect();
        let old_text: String =
            chars.iter().skip(position.1).take(old_text_len).collect();

        let cursor_after = (position.0, position.1 + new_text.chars().count());

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
        // Optimized replacement using replace_range
        buffer.replace_range(
            self.position.0,
            self.position.1,
            self.old_text.chars().count(),
            &self.new_text,
        );

        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        // Restore old text using replace_range
        buffer.replace_range(
            self.position.0,
            self.position.1,
            self.new_text.chars().count(),
            &self.old_text,
        );

        *cursor = self.cursor_before;
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
