//! Canvas focus and IME preedit/commit message handlers.

use iced::Task;

use crate::canvas_editor::{CodeEditor, ImePreedit, Message};

impl CodeEditor {
    /// Handles canvas focus gained event.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_canvas_focus_gained_msg(&mut self) -> Task<Message> {
        self.has_canvas_focus = true;
        self.focus_locked = false; // Unlock focus when gained
        self.show_cursor = true;
        self.reset_cursor_blink();
        self.overlay_cache.clear();
        Task::none()
    }

    /// Handles canvas focus lost event.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_canvas_focus_lost_msg(&mut self) -> Task<Message> {
        self.has_canvas_focus = false;
        self.focus_locked = true; // Lock focus when lost to prevent focus stealing
        self.show_cursor = false;
        self.ime_preedit = None;
        self.overlay_cache.clear();
        Task::none()
    }

    /// Handles IME opened event.
    ///
    /// Clears current preedit content to accept new input.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_ime_opened_msg(&mut self) -> Task<Message> {
        self.ime_preedit = None;
        self.overlay_cache.clear();
        Task::none()
    }

    /// Handles IME preedit event.
    ///
    /// Updates the preedit text and selection while the user is composing.
    ///
    /// # Arguments
    ///
    /// * `content` - The preedit text content
    /// * `selection` - The selection range within the preedit text
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_ime_preedit_msg(
        &mut self,
        content: &str,
        selection: &Option<std::ops::Range<usize>>,
    ) -> Task<Message> {
        if content.is_empty() {
            self.ime_preedit = None;
        } else {
            self.ime_preedit = Some(ImePreedit {
                content: content.to_string(),
                selection: selection.clone(),
            });
        }

        self.overlay_cache.clear();
        Task::none()
    }

    /// Handles IME commit event.
    ///
    /// Inserts the committed text at the cursor position.
    ///
    /// # Arguments
    ///
    /// * `text` - The committed text
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor after insertion
    pub(crate) fn handle_ime_commit_msg(
        &mut self,
        text: &str,
    ) -> Task<Message> {
        self.ime_preedit = None;

        if text.is_empty() || !self.vim_accepts_insert_input() {
            self.overlay_cache.clear();
            return Task::none();
        }

        self.ensure_grouping_started();

        self.paste_text(text);
        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    /// Handles IME closed event.
    ///
    /// Clears preedit state to return to normal input mode.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_ime_closed_msg(&mut self) -> Task<Message> {
        self.ime_preedit = None;
        self.overlay_cache.clear();
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_focus_lost() {
        let mut editor = CodeEditor::new("test", "rs");
        editor.has_canvas_focus = true;

        let _ = editor.update(&Message::CanvasFocusLost);

        assert!(!editor.has_canvas_focus);
        assert!(!editor.show_cursor);
        assert!(editor.focus_locked, "Focus should be locked when lost");
    }

    #[test]
    fn test_canvas_focus_gained_resets_lock() {
        let mut editor = CodeEditor::new("test", "rs");
        editor.has_canvas_focus = false;
        editor.focus_locked = true;

        let _ = editor.update(&Message::CanvasFocusGained);

        assert!(editor.has_canvas_focus);
        assert!(
            !editor.focus_locked,
            "Focus lock should be reset when focus is gained"
        );
    }

    #[test]
    fn test_focus_lock_state() {
        let mut editor = CodeEditor::new("test", "rs");

        // Initially, focus should not be locked
        assert!(!editor.focus_locked);

        // When focus is lost, it should be locked
        let _ = editor.update(&Message::CanvasFocusLost);
        assert!(editor.focus_locked, "Focus should be locked when lost");

        // When focus is regained, it should be unlocked
        editor.request_focus();
        let _ = editor.update(&Message::CanvasFocusGained);
        assert!(!editor.focus_locked, "Focus should be unlocked when regained");

        // Can manually reset focus lock
        editor.focus_locked = true;
        editor.reset_focus_lock();
        assert!(!editor.focus_locked, "Focus lock should be resetable");
    }

    #[test]
    fn test_reset_focus_lock() {
        let mut editor = CodeEditor::new("test", "rs");
        editor.focus_locked = true;

        editor.reset_focus_lock();

        assert!(!editor.focus_locked);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_ime_preedit_and_commit_chinese() {
        let mut editor = CodeEditor::new("", "py");
        // Simulate IME opened
        let _ = editor.update(&Message::ImeOpened);
        assert!(editor.ime_preedit.is_none());

        // Preedit with Chinese content and a selection range
        let content = "安全与合规".to_string();
        let selection = Some(0..3); // range aligned to UTF-8 character boundary
        let _ = editor
            .update(&Message::ImePreedit(content.clone(), selection.clone()));

        assert!(editor.ime_preedit.is_some());
        assert_eq!(
            editor.ime_preedit.as_ref().unwrap().content.clone(),
            content
        );
        assert_eq!(
            editor.ime_preedit.as_ref().unwrap().selection.clone(),
            selection
        );

        // Commit should insert the text and clear preedit
        let _ = editor.update(&Message::ImeCommit("安全与合规".to_string()));
        assert!(editor.ime_preedit.is_none());
        assert_eq!(editor.buffer.line(0), "安全与合规");
        assert_eq!(
            editor.cursors.primary_position(),
            (0, "安全与合规".chars().count())
        );
    }

    #[test]
    fn test_canvas_focus_gained() {
        let mut editor = CodeEditor::new("hello world", "py");
        assert!(!editor.has_canvas_focus);
        assert!(!editor.show_cursor);

        let _ = editor.update(&Message::CanvasFocusGained);

        assert!(editor.has_canvas_focus);
        assert!(editor.show_cursor);
    }
}
