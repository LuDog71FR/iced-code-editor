//! Editor focus management: programmatic focus/blur and the canvas focus
//! lock used to suppress input during focus transitions.

use std::sync::atomic::Ordering;

use crate::canvas_editor::{CodeEditor, FOCUSED_EDITOR_ID};

impl CodeEditor {
    /// Requests focus for this editor.
    ///
    /// This method programmatically sets the focus to this editor instance,
    /// allowing it to receive keyboard events. Other editors will automatically
    /// lose focus.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor1 = CodeEditor::new("fn main() {}", "rs");
    /// let mut editor2 = CodeEditor::new("fn test() {}", "rs");
    ///
    /// // Give focus to editor2
    /// editor2.request_focus();
    /// ```
    pub fn request_focus(&self) {
        FOCUSED_EDITOR_ID.store(self.editor_id, Ordering::Relaxed);
    }

    /// Checks if this editor currently has focus.
    ///
    /// Returns `true` if this editor will receive keyboard events,
    /// `false` otherwise.
    ///
    /// # Returns
    ///
    /// `true` if focused, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// if editor.is_focused() {
    ///     println!("Editor has focus");
    /// }
    /// ```
    pub fn is_focused(&self) -> bool {
        FOCUSED_EDITOR_ID.load(Ordering::Relaxed) == self.editor_id
    }

    /// Removes canvas focus from this editor.
    ///
    /// This method programmatically removes focus from the canvas, preventing
    /// it from receiving keyboard events. The cursor will be hidden, but the
    /// selection will remain visible.
    ///
    /// Call this when focus should move to another widget (e.g., text input).
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.lose_focus();
    /// ```
    pub fn lose_focus(&mut self) {
        self.has_canvas_focus = false;
        self.show_cursor = false;
        self.ime_preedit = None;
    }

    /// Resets the focus lock state.
    ///
    /// This method can be called to manually unlock focus processing
    /// after a focus transition has completed. This is useful when
    /// you want to allow the editor to process input again after
    /// programmatic focus changes.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.reset_focus_lock();
    /// ```
    pub fn reset_focus_lock(&mut self) {
        self.focus_locked = false;
    }
}
