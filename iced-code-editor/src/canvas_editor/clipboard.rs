//! Clipboard operations (copy, paste, delete selection).

use iced::Task;

use super::command::{Command, DeleteRangeCommand, InsertTextCommand};
use super::{CodeEditor, Message};

impl CodeEditor {
    /// Copies selected text to clipboard.
    pub(crate) fn copy_selection(&self) -> Task<Message> {
        if let Some(text) = self.get_selected_text() {
            iced::clipboard::write(text)
        } else {
            Task::none()
        }
    }

    /// Deletes the selected text.
    pub(crate) fn delete_selection(&mut self) {
        if let Some((start, end)) = self.get_selection_range() {
            if start == end {
                return; // No selection
            }

            let mut cmd =
                DeleteRangeCommand::new(&self.buffer, start, end, self.cursor);
            cmd.execute(&mut self.buffer, &mut self.cursor);
            self.history.push(Box::new(cmd));
            self.clear_selection();
        }
    }

    /// Pastes text from clipboard at cursor position.
    pub(crate) fn paste_text(&mut self, text: &str) {
        // If there's a selection, delete it first
        if self.selection_start.is_some() && self.selection_end.is_some() {
            self.delete_selection();
        }

        let (line, col) = self.cursor;
        let mut cmd =
            InsertTextCommand::new(line, col, text.to_string(), self.cursor);
        cmd.execute(&mut self.buffer, &mut self.cursor);
        self.history.push(Box::new(cmd));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_selection_single_line() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        editor.delete_selection();
        assert_eq!(editor.buffer.line(0), " world");
    }
}
