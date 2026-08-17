//! State for the go-to-line dialog.

pub(crate) mod dialog;
mod update;

use iced::widget::Id;

use crate::canvas_editor::{CodeEditor, Message};

/// State owned by the compact go-to-line input.
#[derive(Debug, Clone)]
pub(crate) struct GotoLineState {
    /// User-entered, one-based line number.
    pub(crate) query: String,
    /// Whether the dialog is visible.
    pub(crate) is_open: bool,
    /// Stable input ID used for focus and selection operations.
    pub(crate) input_id: Id,
}

impl Default for GotoLineState {
    fn default() -> Self {
        Self { query: String::new(), is_open: false, input_id: Id::unique() }
    }
}

impl GotoLineState {
    /// Creates a closed go-to-line state.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Opens the dialog and pre-fills the current one-based line number.
    pub(crate) fn open(&mut self, current_line: usize) {
        self.query = current_line.saturating_add(1).to_string();
        self.is_open = true;
    }

    /// Closes the dialog without changing its current query.
    pub(crate) fn close(&mut self) {
        self.is_open = false;
    }

    /// Returns the entered one-based line number when it is a positive integer.
    pub(crate) fn target_line(&self) -> Option<usize> {
        self.query.trim().parse::<usize>().ok().filter(|line| *line > 0)
    }
}

impl CodeEditor {
    /// Opens the go-to-line dialog programmatically.
    ///
    /// The input is pre-filled with the current one-based line number. Use
    /// this to wire a "Go to Line…" menu item, alongside the built-in
    /// `Ctrl/Cmd+G` shortcut.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses and selects the dialog's input
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
    /// let _task = editor.open_goto_line_dialog();
    /// ```
    pub fn open_goto_line_dialog(&mut self) -> iced::Task<Message> {
        self.update(&Message::OpenGotoLine)
    }

    /// Closes the go-to-line dialog programmatically.
    ///
    /// Leaves the cursor where it is. Safe to call when the dialog is already
    /// closed.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` returning focus to the editor canvas
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
    /// let _task = editor.open_goto_line_dialog();
    /// let _task = editor.close_goto_line_dialog();
    /// ```
    pub fn close_goto_line_dialog(&mut self) -> iced::Task<Message> {
        self.update(&Message::CloseGotoLine)
    }
}
