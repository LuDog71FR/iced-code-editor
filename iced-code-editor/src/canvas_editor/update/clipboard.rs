//! Cut, select-all, and paste message handlers.

use iced::Task;

use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Cuts all selected ranges to the clipboard as a single undoable edit.
    pub(super) fn handle_cut_msg(&mut self) -> Task<Message> {
        if !self.cursors.iter().any(|cursor| cursor.has_selection()) {
            return Task::none();
        }

        self.end_grouping_if_active();
        self.ensure_grouping_started();
        let clipboard_task = self.copy_selection();
        self.delete_selection();
        self.end_grouping_if_active();
        self.finish_edit_operation();

        Task::batch([clipboard_task, self.scroll_to_cursor()])
    }

    /// Selects the complete document.
    pub(super) fn handle_select_all_msg(&mut self) -> Task<Message> {
        self.end_grouping_if_active();

        let last_line = self.buffer.line_count().saturating_sub(1);
        let end = (last_line, self.buffer.line_len(last_line));
        self.cursors.set_single(end);
        self.cursors.primary_mut().anchor = Some((0, 0));
        self.overlay_cache.clear();
        self.reset_cursor_blink();

        self.scroll_to_cursor()
    }

    /// Handles paste operations.
    ///
    /// If the provided text is empty, reads from clipboard. Otherwise pastes
    /// the provided text at the cursor position.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to paste (empty string triggers clipboard read)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that may read clipboard or scroll to cursor
    pub(super) fn handle_paste_msg(&mut self, text: &str) -> Task<Message> {
        // End grouping on paste
        self.end_grouping_if_active();

        // If text is empty, we need to read from clipboard
        if text.is_empty() {
            // Return a task that reads clipboard and chains to paste
            iced::clipboard::read().and_then(|clipboard_text| {
                Task::done(Message::Paste(clipboard_text))
            })
        } else {
            // We have the text, paste it. `paste_text` already pushes a
            // single command on its single-cursor fast path; group the
            // multi-cursor path's per-cursor commands (and any selection
            // deletions it performs first) into one composite, so a single
            // undo restores every cursor's paste instead of just the last.
            let multi = self.cursors.len() > 1;
            if multi {
                self.ensure_grouping_started();
            }
            self.paste_text(text);
            if multi {
                self.end_grouping_if_active();
            }
            self.finish_edit_operation();
            self.scroll_to_cursor()
        }
    }
}
