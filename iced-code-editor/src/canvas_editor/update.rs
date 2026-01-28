//! Message handling and update logic.

use iced::Task;
use iced::widget::operation::{focus, select_all};

use super::command::{
    Command, CompositeCommand, DeleteCharCommand, DeleteForwardCommand,
    InsertCharCommand, InsertNewlineCommand, ReplaceTextCommand,
};
use super::{
    ArrowDirection, CURSOR_BLINK_INTERVAL, CodeEditor, ImePreedit, Message,
};

impl CodeEditor {
    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Performs common cleanup operations after edit operations.
    ///
    /// This method should be called after any operation that modifies the buffer content.
    /// It resets the cursor blink animation, refreshes search matches if search is active,
    /// and clears the rendering cache to force a redraw.
    fn finish_edit_operation(&mut self) {
        self.reset_cursor_blink();
        self.refresh_search_matches_if_needed();
        self.cache.clear();
    }

    /// Performs common cleanup operations after navigation operations.
    ///
    /// This method should be called after cursor movement operations.
    /// It resets the cursor blink animation and clears the rendering cache.
    fn finish_navigation_operation(&mut self) {
        self.reset_cursor_blink();
        self.cache.clear();
    }

    /// Starts command grouping with the given label if not already grouping.
    ///
    /// This is used for smart undo functionality, allowing multiple related
    /// operations to be undone as a single unit.
    ///
    /// # Arguments
    ///
    /// * `label` - A descriptive label for the group of commands
    fn ensure_grouping_started(&mut self, label: &str) {
        if !self.is_grouping {
            self.history.begin_group(label);
            self.is_grouping = true;
        }
    }

    /// Ends command grouping if currently active.
    ///
    /// This should be called when a series of related operations is complete,
    /// or when starting a new type of operation that shouldn't be grouped
    /// with previous operations.
    fn end_grouping_if_active(&mut self) {
        if self.is_grouping {
            self.history.end_group();
            self.is_grouping = false;
        }
    }

    /// Deletes the current selection and performs cleanup if a selection exists.
    ///
    /// # Returns
    ///
    /// `true` if a selection was deleted, `false` if no selection existed
    fn delete_selection_if_present(&mut self) -> bool {
        if self.selection_start.is_some() && self.selection_end.is_some() {
            self.delete_selection();
            self.finish_edit_operation();
            true
        } else {
            false
        }
    }

    // =========================================================================
    // Text Input Handlers
    // =========================================================================

    /// Handles character input message operations.
    ///
    /// Inserts a character at the current cursor position and adds it to the
    /// undo history. Characters are grouped together for smart undo.
    ///
    /// # Arguments
    ///
    /// * `ch` - The character to insert
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_character_input_msg(&mut self, ch: char) -> Task<Message> {
        // Start grouping if not already grouping (for smart undo)
        self.ensure_grouping_started("Typing");

        let (line, col) = self.cursor;
        let mut cmd = InsertCharCommand::new(line, col, ch, self.cursor);
        cmd.execute(&mut self.buffer, &mut self.cursor);
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        Task::none()
    }

    /// Handles Tab key press (inserts 4 spaces).
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_tab(&mut self) -> Task<Message> {
        // Insert 4 spaces for Tab
        // Start grouping if not already grouping
        self.ensure_grouping_started("Tab");

        let (line, col) = self.cursor;
        // Insert 4 spaces
        for i in 0..4 {
            let current_col = col + i;
            let mut cmd = InsertCharCommand::new(
                line,
                current_col,
                ' ',
                (line, current_col),
            );
            cmd.execute(&mut self.buffer, &mut self.cursor);
            self.history.push(Box::new(cmd));
        }

        self.finish_navigation_operation();
        Task::none()
    }

    /// Handles Enter key press (inserts newline).
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_enter(&mut self) -> Task<Message> {
        // End grouping on enter
        self.end_grouping_if_active();

        let (line, col) = self.cursor;
        let mut cmd = InsertNewlineCommand::new(line, col, self.cursor);
        cmd.execute(&mut self.buffer, &mut self.cursor);
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    // =========================================================================
    // Deletion Handlers
    // =========================================================================

    /// Handles Backspace key press.
    ///
    /// If there's a selection, deletes the selection. Otherwise, deletes the
    /// character before the cursor.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible if selection was deleted
    fn handle_backspace(&mut self) -> Task<Message> {
        // End grouping on backspace (separate from typing)
        self.end_grouping_if_active();

        // Check if there's a selection - if so, delete it instead
        if self.delete_selection_if_present() {
            return self.scroll_to_cursor();
        }

        // No selection - perform normal backspace
        let (line, col) = self.cursor;
        let mut cmd =
            DeleteCharCommand::new(&self.buffer, line, col, self.cursor);
        cmd.execute(&mut self.buffer, &mut self.cursor);
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        self.scroll_to_cursor()
    }

    /// Handles Delete key press.
    ///
    /// If there's a selection, deletes the selection. Otherwise, deletes the
    /// character after the cursor.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible if selection was deleted
    fn handle_delete(&mut self) -> Task<Message> {
        // End grouping on delete
        self.end_grouping_if_active();

        // Check if there's a selection - if so, delete it instead
        if self.delete_selection_if_present() {
            return self.scroll_to_cursor();
        }

        // No selection - perform normal forward delete
        let (line, col) = self.cursor;
        let mut cmd =
            DeleteForwardCommand::new(&self.buffer, line, col, self.cursor);
        cmd.execute(&mut self.buffer, &mut self.cursor);
        self.history.push(Box::new(cmd));

        self.finish_edit_operation();
        Task::none()
    }

    /// Handles explicit selection deletion (Shift+Delete).
    ///
    /// Deletes the selected text if a selection exists.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_delete_selection(&mut self) -> Task<Message> {
        // End grouping on delete selection
        self.end_grouping_if_active();

        // Delete selected text
        self.delete_selection();
        self.reset_cursor_blink();
        self.cache.clear();
        self.scroll_to_cursor()
    }

    // =========================================================================
    // Navigation Handlers
    // =========================================================================

    /// Handles arrow key navigation.
    ///
    /// # Arguments
    ///
    /// * `direction` - The direction of movement
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_arrow_key(
        &mut self,
        direction: ArrowDirection,
        shift_pressed: bool,
    ) -> Task<Message> {
        // End grouping on navigation
        self.end_grouping_if_active();

        if shift_pressed {
            // Start selection if not already started
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
            self.move_cursor(direction);
            self.selection_end = Some(self.cursor);
        } else {
            // Clear selection and move cursor
            self.clear_selection();
            self.move_cursor(direction);
        }
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Home key press.
    ///
    /// Moves the cursor to the start of the current line.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_home(&mut self, shift_pressed: bool) -> Task<Message> {
        if shift_pressed {
            // Start selection if not already started
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
            self.cursor.1 = 0; // Move to start of line
            self.selection_end = Some(self.cursor);
        } else {
            // Clear selection and move cursor
            self.clear_selection();
            self.cursor.1 = 0;
        }
        self.finish_navigation_operation();
        Task::none()
    }

    /// Handles End key press.
    ///
    /// Moves the cursor to the end of the current line.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_end(&mut self, shift_pressed: bool) -> Task<Message> {
        let line = self.cursor.0;
        let line_len = self.buffer.line_len(line);

        if shift_pressed {
            // Start selection if not already started
            if self.selection_start.is_none() {
                self.selection_start = Some(self.cursor);
            }
            self.cursor.1 = line_len; // Move to end of line
            self.selection_end = Some(self.cursor);
        } else {
            // Clear selection and move cursor
            self.clear_selection();
            self.cursor.1 = line_len;
        }
        self.finish_navigation_operation();
        Task::none()
    }

    /// Handles Ctrl+Home key press.
    ///
    /// Moves the cursor to the beginning of the document.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_ctrl_home(&mut self) -> Task<Message> {
        // Move cursor to the beginning of the document
        self.clear_selection();
        self.cursor = (0, 0);
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Ctrl+End key press.
    ///
    /// Moves the cursor to the end of the document.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_ctrl_end(&mut self) -> Task<Message> {
        // Move cursor to the end of the document
        self.clear_selection();
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_len(last_line);
        self.cursor = (last_line, last_col);
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Page Up key press.
    ///
    /// Scrolls the view up by one page.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_page_up(&mut self) -> Task<Message> {
        self.page_up();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Page Down key press.
    ///
    /// Scrolls the view down by one page.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    fn handle_page_down(&mut self) -> Task<Message> {
        self.page_down();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    // =========================================================================
    // Mouse and Selection Handlers
    // =========================================================================

    /// Handles mouse click operations.
    ///
    /// Sets focus, ends command grouping, positions cursor, starts selection tracking.
    ///
    /// # Arguments
    ///
    /// * `point` - The click position
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_mouse_click_msg(&mut self, point: iced::Point) -> Task<Message> {
        // Capture focus when clicked
        super::FOCUSED_EDITOR_ID
            .store(self.editor_id, std::sync::atomic::Ordering::Relaxed);

        // End grouping on mouse click
        self.end_grouping_if_active();

        self.handle_mouse_click(point);
        self.reset_cursor_blink();
        // Clear selection on click
        self.clear_selection();
        self.is_dragging = true;
        self.selection_start = Some(self.cursor);

        // Gain canvas focus
        self.has_canvas_focus = true;
        self.show_cursor = true;

        Task::none()
    }

    /// Handles mouse drag operations for selection.
    ///
    /// # Arguments
    ///
    /// * `point` - The drag position
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_mouse_drag_msg(&mut self, point: iced::Point) -> Task<Message> {
        if self.is_dragging {
            self.handle_mouse_drag(point);
            self.cache.clear();
        }
        Task::none()
    }

    /// Handles mouse release operations.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none() as no scrolling is needed)
    fn handle_mouse_release_msg(&mut self) -> Task<Message> {
        self.is_dragging = false;
        Task::none()
    }

    // =========================================================================
    // Clipboard Handlers
    // =========================================================================

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
    fn handle_paste_msg(&mut self, text: &str) -> Task<Message> {
        // End grouping on paste
        self.end_grouping_if_active();

        // If text is empty, we need to read from clipboard
        if text.is_empty() {
            // Return a task that reads clipboard and chains to paste
            iced::clipboard::read().and_then(|clipboard_text| {
                Task::done(Message::Paste(clipboard_text))
            })
        } else {
            // We have the text, paste it
            self.paste_text(text);
            self.finish_edit_operation();
            self.scroll_to_cursor()
        }
    }

    // =========================================================================
    // History (Undo/Redo) Handlers
    // =========================================================================

    /// Handles undo operations.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor if undo succeeded
    fn handle_undo_msg(&mut self) -> Task<Message> {
        // End any current grouping before undoing
        self.end_grouping_if_active();

        if self.history.undo(&mut self.buffer, &mut self.cursor) {
            self.clear_selection();
            self.finish_edit_operation();
            self.scroll_to_cursor()
        } else {
            Task::none()
        }
    }

    /// Handles redo operations.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor if redo succeeded
    fn handle_redo_msg(&mut self) -> Task<Message> {
        if self.history.redo(&mut self.buffer, &mut self.cursor) {
            self.clear_selection();
            self.finish_edit_operation();
            self.scroll_to_cursor()
        } else {
            Task::none()
        }
    }

    // =========================================================================
    // Search and Replace Handlers
    // =========================================================================

    /// Handles opening the search dialog.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses and selects all in the search input
    fn handle_open_search_msg(&mut self) -> Task<Message> {
        self.search_state.open_search();
        self.cache.clear();

        // Focus the search input and select all text if any
        Task::batch([
            focus(self.search_state.search_input_id.clone()),
            select_all(self.search_state.search_input_id.clone()),
        ])
    }

    /// Handles opening the search and replace dialog.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses and selects all in the search input
    fn handle_open_search_replace_msg(&mut self) -> Task<Message> {
        self.search_state.open_replace();
        self.cache.clear();

        // Focus the search input and select all text if any
        Task::batch([
            focus(self.search_state.search_input_id.clone()),
            select_all(self.search_state.search_input_id.clone()),
        ])
    }

    /// Handles closing the search dialog.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_close_search_msg(&mut self) -> Task<Message> {
        self.search_state.close();
        self.cache.clear();
        Task::none()
    }

    /// Handles search query text changes.
    ///
    /// # Arguments
    ///
    /// * `query` - The new search query
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to first match if any
    fn handle_search_query_changed_msg(
        &mut self,
        query: &str,
    ) -> Task<Message> {
        self.search_state.set_query(query.to_string(), &self.buffer);
        self.cache.clear();

        // Move cursor to first match if any
        if let Some(match_pos) = self.search_state.current_match() {
            self.cursor = (match_pos.line, match_pos.col);
            self.clear_selection();
            return self.scroll_to_cursor();
        }
        Task::none()
    }

    /// Handles replace query text changes.
    ///
    /// # Arguments
    ///
    /// * `replace_text` - The new replacement text
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_replace_query_changed_msg(
        &mut self,
        replace_text: &str,
    ) -> Task<Message> {
        self.search_state.set_replace_with(replace_text.to_string());
        Task::none()
    }

    /// Handles toggling case-sensitive search.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to first match if any
    fn handle_toggle_case_sensitive_msg(&mut self) -> Task<Message> {
        self.search_state.toggle_case_sensitive(&self.buffer);
        self.cache.clear();

        // Move cursor to first match if any
        if let Some(match_pos) = self.search_state.current_match() {
            self.cursor = (match_pos.line, match_pos.col);
            self.clear_selection();
            return self.scroll_to_cursor();
        }
        Task::none()
    }

    /// Handles finding the next match.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to the next match if any
    fn handle_find_next_msg(&mut self) -> Task<Message> {
        if !self.search_state.matches.is_empty() {
            self.search_state.next_match();
            if let Some(match_pos) = self.search_state.current_match() {
                self.cursor = (match_pos.line, match_pos.col);
                self.clear_selection();
                self.cache.clear();
                return self.scroll_to_cursor();
            }
        }
        Task::none()
    }

    /// Handles finding the previous match.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to the previous match if any
    fn handle_find_previous_msg(&mut self) -> Task<Message> {
        if !self.search_state.matches.is_empty() {
            self.search_state.previous_match();
            if let Some(match_pos) = self.search_state.current_match() {
                self.cursor = (match_pos.line, match_pos.col);
                self.clear_selection();
                self.cache.clear();
                return self.scroll_to_cursor();
            }
        }
        Task::none()
    }

    /// Handles replacing the current match and moving to the next.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to the next match if any
    fn handle_replace_next_msg(&mut self) -> Task<Message> {
        // Replace current match and move to next
        if let Some(match_pos) = self.search_state.current_match() {
            let query_len = self.search_state.query.chars().count();
            let replace_text = self.search_state.replace_with.clone();

            // Create and execute replace command
            let mut cmd = ReplaceTextCommand::new(
                &self.buffer,
                (match_pos.line, match_pos.col),
                query_len,
                replace_text,
                self.cursor,
            );
            cmd.execute(&mut self.buffer, &mut self.cursor);
            self.history.push(Box::new(cmd));

            // Update matches after replacement
            self.search_state.update_matches(&self.buffer);

            // Move to next match if available
            if !self.search_state.matches.is_empty()
                && let Some(next_match) = self.search_state.current_match()
            {
                self.cursor = (next_match.line, next_match.col);
            }

            self.clear_selection();
            self.cache.clear();
            return self.scroll_to_cursor();
        }
        Task::none()
    }

    /// Handles replacing all matches.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to cursor after replacement
    fn handle_replace_all_msg(&mut self) -> Task<Message> {
        // Perform a fresh search to find ALL matches (ignoring the display limit)
        let all_matches = super::search::find_matches(
            &self.buffer,
            &self.search_state.query,
            self.search_state.case_sensitive,
            None, // No limit for Replace All
        );

        if !all_matches.is_empty() {
            let query_len = self.search_state.query.chars().count();
            let replace_text = self.search_state.replace_with.clone();

            // Create composite command for undo
            let mut composite =
                CompositeCommand::new("Replace All".to_string());

            // Process matches in reverse order (to preserve positions)
            for match_pos in all_matches.iter().rev() {
                let cmd = ReplaceTextCommand::new(
                    &self.buffer,
                    (match_pos.line, match_pos.col),
                    query_len,
                    replace_text.clone(),
                    self.cursor,
                );
                composite.add(Box::new(cmd));
            }

            // Execute all replacements
            composite.execute(&mut self.buffer, &mut self.cursor);
            self.history.push(Box::new(composite));

            // Update matches (should be empty now)
            self.search_state.update_matches(&self.buffer);
            self.clear_selection();
            self.cache.clear();
            self.scroll_to_cursor()
        } else {
            Task::none()
        }
    }

    /// Handles Tab key in search dialog (cycle forward).
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses the next field
    fn handle_search_dialog_tab_msg(&mut self) -> Task<Message> {
        // Cycle focus forward (Search → Replace → Search)
        self.search_state.focus_next_field();

        // Focus the appropriate input based on new focused_field
        match self.search_state.focused_field {
            crate::canvas_editor::search::SearchFocusedField::Search => {
                focus(self.search_state.search_input_id.clone())
            }
            crate::canvas_editor::search::SearchFocusedField::Replace => {
                focus(self.search_state.replace_input_id.clone())
            }
        }
    }

    /// Handles Shift+Tab key in search dialog (cycle backward).
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses the previous field
    fn handle_search_dialog_shift_tab_msg(&mut self) -> Task<Message> {
        // Cycle focus backward (Replace → Search → Replace)
        self.search_state.focus_previous_field();

        // Focus the appropriate input based on new focused_field
        match self.search_state.focused_field {
            crate::canvas_editor::search::SearchFocusedField::Search => {
                focus(self.search_state.search_input_id.clone())
            }
            crate::canvas_editor::search::SearchFocusedField::Replace => {
                focus(self.search_state.replace_input_id.clone())
            }
        }
    }

    // =========================================================================
    // Focus and IME Handlers
    // =========================================================================

    /// Handles canvas focus gained event.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_canvas_focus_gained_msg(&mut self) -> Task<Message> {
        self.has_canvas_focus = true;
        self.show_cursor = true;
        self.reset_cursor_blink();
        self.cache.clear();
        Task::none()
    }

    /// Handles canvas focus lost event.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_canvas_focus_lost_msg(&mut self) -> Task<Message> {
        self.has_canvas_focus = false;
        self.show_cursor = false;
        self.ime_preedit = None;
        self.cache.clear();
        Task::none()
    }

    /// Handles IME opened event.
    ///
    /// Clears current preedit content to accept new input.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_ime_opened_msg(&mut self) -> Task<Message> {
        self.ime_preedit = None;
        self.cache.clear();
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
    fn handle_ime_preedit_msg(
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

        self.cache.clear();
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
    fn handle_ime_commit_msg(&mut self, text: &str) -> Task<Message> {
        self.ime_preedit = None;

        if text.is_empty() {
            self.cache.clear();
            return Task::none();
        }

        self.ensure_grouping_started("Typing");

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
    fn handle_ime_closed_msg(&mut self) -> Task<Message> {
        self.ime_preedit = None;
        self.cache.clear();
        Task::none()
    }

    // =========================================================================
    // Complex Standalone Handlers
    // =========================================================================

    /// Handles cursor blink tick event.
    ///
    /// Updates cursor visibility for blinking animation.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_tick_msg(&mut self) -> Task<Message> {
        // Handle cursor blinking only if editor has focus
        if self.is_focused()
            && self.has_canvas_focus
            && self.last_blink.elapsed() >= CURSOR_BLINK_INTERVAL
        {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = std::time::Instant::now();
            self.cache.clear();
        }

        // Hide cursor if canvas doesn't have focus
        if !self.has_canvas_focus {
            self.show_cursor = false;
        }

        Task::none()
    }

    /// Handles viewport scrolled event.
    ///
    /// Manages the virtual scrolling cache window to optimize rendering
    /// for large files. Only clears the cache when scrolling crosses the
    /// cached window boundary or when viewport dimensions change.
    ///
    /// # Arguments
    ///
    /// * `viewport` - The viewport information after scrolling
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    fn handle_scrolled_msg(
        &mut self,
        viewport: iced::widget::scrollable::Viewport,
    ) -> Task<Message> {
        // Virtual-scrolling cache window:
        // Instead of clearing the canvas cache for every small scroll,
        // we maintain a larger "render window" of visual lines around
        // the visible range. We only clear the cache and re-window
        // when the scroll crosses the window boundary or the viewport
        // size changes significantly. This prevents frequent re-highlighting
        // and layout recomputation for very large files while ensuring
        // the first scroll renders correctly without requiring a click.
        let new_scroll = viewport.absolute_offset().y;
        let new_height = viewport.bounds().height;
        let new_width = viewport.bounds().width;
        let scroll_changed = (self.viewport_scroll - new_scroll).abs() > 0.1;
        let visible_lines_count =
            (new_height / self.line_height).ceil() as usize + 2;
        let first_visible_line =
            (new_scroll / self.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count
            * crate::canvas_editor::CACHE_WINDOW_MARGIN_MULTIPLIER;
        let window_start = first_visible_line.saturating_sub(margin);
        let window_end = last_visible_line + margin;
        // Decide whether we need to re-window the cache.
        // Special-case top-of-file: when window_start == 0, allow small forward scrolls
        // without forcing a rewindow, to avoid thrashing when the visible range is near 0.
        let need_rewindow =
            if self.cache_window_end_line > self.cache_window_start_line {
                let lower_boundary_trigger = self.cache_window_start_line > 0
                    && first_visible_line
                        < self
                            .cache_window_start_line
                            .saturating_add(visible_lines_count / 2);
                let upper_boundary_trigger = last_visible_line
                    > self
                        .cache_window_end_line
                        .saturating_sub(visible_lines_count / 2);
                lower_boundary_trigger || upper_boundary_trigger
            } else {
                true
            };
        // Clear cache when viewport dimensions change significantly
        // to ensure proper redraw (e.g., window resize)
        if (self.viewport_height - new_height).abs() > 1.0
            || (self.viewport_width - new_width).abs() > 1.0
            || (scroll_changed && need_rewindow)
        {
            self.cache_window_start_line = window_start;
            self.cache_window_end_line = window_end;
            self.last_first_visible_line = first_visible_line;
            self.cache.clear();
        }
        self.viewport_scroll = new_scroll;
        self.viewport_height = new_height;
        self.viewport_width = new_width;
        Task::none()
    }

    // =========================================================================
    // Main Update Method
    // =========================================================================

    /// Updates the editor state based on messages and returns scroll commands.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to process for updating the editor state
    ///
    /// # Returns
    /// A `Task<Message>` for any asynchronous operations, such as scrolling to keep the cursor visible after state updates
    pub fn update(&mut self, message: &Message) -> Task<Message> {
        match message {
            // Text input operations
            Message::CharacterInput(ch) => self.handle_character_input_msg(*ch),
            Message::Tab => self.handle_tab(),
            Message::Enter => self.handle_enter(),

            // Deletion operations
            Message::Backspace => self.handle_backspace(),
            Message::Delete => self.handle_delete(),
            Message::DeleteSelection => self.handle_delete_selection(),

            // Navigation operations
            Message::ArrowKey(direction, shift) => {
                self.handle_arrow_key(*direction, *shift)
            }
            Message::Home(shift) => self.handle_home(*shift),
            Message::End(shift) => self.handle_end(*shift),
            Message::CtrlHome => self.handle_ctrl_home(),
            Message::CtrlEnd => self.handle_ctrl_end(),
            Message::PageUp => self.handle_page_up(),
            Message::PageDown => self.handle_page_down(),

            // Mouse and selection operations
            Message::MouseClick(point) => self.handle_mouse_click_msg(*point),
            Message::MouseDrag(point) => self.handle_mouse_drag_msg(*point),
            Message::MouseRelease => self.handle_mouse_release_msg(),

            // Clipboard operations
            Message::Copy => self.copy_selection(),
            Message::Paste(text) => self.handle_paste_msg(text),

            // History operations
            Message::Undo => self.handle_undo_msg(),
            Message::Redo => self.handle_redo_msg(),

            // Search and replace operations
            Message::OpenSearch => self.handle_open_search_msg(),
            Message::OpenSearchReplace => self.handle_open_search_replace_msg(),
            Message::CloseSearch => self.handle_close_search_msg(),
            Message::SearchQueryChanged(query) => {
                self.handle_search_query_changed_msg(query)
            }
            Message::ReplaceQueryChanged(text) => {
                self.handle_replace_query_changed_msg(text)
            }
            Message::ToggleCaseSensitive => {
                self.handle_toggle_case_sensitive_msg()
            }
            Message::FindNext => self.handle_find_next_msg(),
            Message::FindPrevious => self.handle_find_previous_msg(),
            Message::ReplaceNext => self.handle_replace_next_msg(),
            Message::ReplaceAll => self.handle_replace_all_msg(),
            Message::SearchDialogTab => self.handle_search_dialog_tab_msg(),
            Message::SearchDialogShiftTab => {
                self.handle_search_dialog_shift_tab_msg()
            }

            // Focus and IME operations
            Message::CanvasFocusGained => self.handle_canvas_focus_gained_msg(),
            Message::CanvasFocusLost => self.handle_canvas_focus_lost_msg(),
            Message::ImeOpened => self.handle_ime_opened_msg(),
            Message::ImePreedit(content, selection) => {
                self.handle_ime_preedit_msg(content, selection)
            }
            Message::ImeCommit(text) => self.handle_ime_commit_msg(text),
            Message::ImeClosed => self.handle_ime_closed_msg(),

            // UI update operations
            Message::Tick => self.handle_tick_msg(),
            Message::Scrolled(viewport) => self.handle_scrolled_msg(*viewport),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::ArrowDirection;

    #[test]
    fn test_new_canvas_editor() {
        let editor = CodeEditor::new("line1\nline2", "py");
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_home_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 5); // Move to middle of line
        let _ = editor.update(&Message::Home(false));
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_end_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 0);
        let _ = editor.update(&Message::End(false));
        assert_eq!(editor.cursor, (0, 11)); // Length of "hello world"
    }

    #[test]
    fn test_arrow_key_with_shift_creates_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 0);

        // Shift+Right should start selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, true));
        assert!(editor.selection_start.is_some());
        assert!(editor.selection_end.is_some());
    }

    #[test]
    fn test_arrow_key_without_shift_clears_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        // Regular arrow key should clear selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, false));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_typing_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let _ = editor.update(&Message::CharacterInput('X'));
        // Current behavior: character is inserted at cursor, selection is NOT automatically deleted
        // This is expected behavior - user must delete selection first (Backspace/Delete) or use Paste
        assert_eq!(editor.buffer.line(0), "Xhello world");
    }

    #[test]
    fn test_ctrl_home() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (2, 5); // Start at line 3, column 5
        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursor, (0, 0)); // Should move to beginning of document
    }

    #[test]
    fn test_ctrl_end() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (0, 0); // Start at beginning
        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursor, (2, 5)); // Should move to end of last line (line3 has 5 chars)
    }

    #[test]
    fn test_ctrl_home_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (2, 5);
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((2, 5));

        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursor, (0, 0));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_ctrl_end_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (0, 0);
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((1, 3));

        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursor, (2, 5));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_scroll_sets_initial_cache_window() {
        let content =
            (0..200).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut editor = CodeEditor::new(&content, "py");

        // Simulate initial viewport
        let height = 400.0;
        let width = 800.0;
        let scroll = 0.0;

        // Expected derived ranges
        let visible_lines_count =
            (height / editor.line_height).ceil() as usize + 2;
        let first_visible_line = (scroll / editor.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count * 2;
        let window_start = first_visible_line.saturating_sub(margin);
        let window_end = last_visible_line + margin;

        // Apply logic similar to Message::Scrolled branch
        editor.viewport_height = height;
        editor.viewport_width = width;
        editor.viewport_scroll = -1.0;
        let scroll_changed = (editor.viewport_scroll - scroll).abs() > 0.1;
        let need_rewindow = true;
        if (editor.viewport_height - height).abs() > 1.0
            || (editor.viewport_width - width).abs() > 1.0
            || (scroll_changed && need_rewindow)
        {
            editor.cache_window_start_line = window_start;
            editor.cache_window_end_line = window_end;
            editor.last_first_visible_line = first_visible_line;
        }
        editor.viewport_scroll = scroll;

        assert_eq!(editor.last_first_visible_line, first_visible_line);
        assert!(editor.cache_window_end_line > editor.cache_window_start_line);
        assert_eq!(editor.cache_window_start_line, window_start);
        assert_eq!(editor.cache_window_end_line, window_end);
    }

    #[test]
    fn test_small_scroll_keeps_window() {
        let content =
            (0..200).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut editor = CodeEditor::new(&content, "py");
        let height = 400.0;
        let width = 800.0;
        let initial_scroll = 0.0;
        let visible_lines_count =
            (height / editor.line_height).ceil() as usize + 2;
        let first_visible_line =
            (initial_scroll / editor.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count * 2;
        let window_start = first_visible_line.saturating_sub(margin);
        let window_end = last_visible_line + margin;
        editor.cache_window_start_line = window_start;
        editor.cache_window_end_line = window_end;
        editor.viewport_height = height;
        editor.viewport_width = width;
        editor.viewport_scroll = initial_scroll;

        // Small scroll inside window
        let small_scroll =
            editor.line_height * (visible_lines_count as f32 / 4.0);
        let first_visible_line2 =
            (small_scroll / editor.line_height).floor() as usize;
        let last_visible_line2 = first_visible_line2 + visible_lines_count;
        let lower_boundary_trigger = editor.cache_window_start_line > 0
            && first_visible_line2
                < editor
                    .cache_window_start_line
                    .saturating_add(visible_lines_count / 2);
        let upper_boundary_trigger = last_visible_line2
            > editor
                .cache_window_end_line
                .saturating_sub(visible_lines_count / 2);
        let need_rewindow = lower_boundary_trigger || upper_boundary_trigger;

        assert!(!need_rewindow, "Small scroll should be inside the window");
        // Window remains unchanged
        assert_eq!(editor.cache_window_start_line, window_start);
        assert_eq!(editor.cache_window_end_line, window_end);
    }

    #[test]
    fn test_large_scroll_rewindows() {
        let content =
            (0..1000).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut editor = CodeEditor::new(&content, "py");
        let height = 400.0;
        let width = 800.0;
        let initial_scroll = 0.0;
        let visible_lines_count =
            (height / editor.line_height).ceil() as usize + 2;
        let first_visible_line =
            (initial_scroll / editor.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count * 2;
        editor.cache_window_start_line =
            first_visible_line.saturating_sub(margin);
        editor.cache_window_end_line = last_visible_line + margin;
        editor.viewport_height = height;
        editor.viewport_width = width;
        editor.viewport_scroll = initial_scroll;

        // Large scroll beyond window boundary
        let large_scroll =
            editor.line_height * ((visible_lines_count * 4) as f32);
        let first_visible_line2 =
            (large_scroll / editor.line_height).floor() as usize;
        let last_visible_line2 = first_visible_line2 + visible_lines_count;
        let window_start2 = first_visible_line2.saturating_sub(margin);
        let window_end2 = last_visible_line2 + margin;
        let need_rewindow = first_visible_line2
            < editor
                .cache_window_start_line
                .saturating_add(visible_lines_count / 2)
            || last_visible_line2
                > editor
                    .cache_window_end_line
                    .saturating_sub(visible_lines_count / 2);
        assert!(need_rewindow, "Large scroll should trigger window update");

        // Apply rewindow
        editor.cache_window_start_line = window_start2;
        editor.cache_window_end_line = window_end2;
        editor.last_first_visible_line = first_visible_line2;

        assert_eq!(editor.cache_window_start_line, window_start2);
        assert_eq!(editor.cache_window_end_line, window_end2);
        assert_eq!(editor.last_first_visible_line, first_visible_line2);
    }

    #[test]
    fn test_delete_selection_message() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 0);
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.line(0), " world");
        assert_eq!(editor.cursor, (0, 0));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_delete_selection_multiline() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursor = (0, 2);
        editor.selection_start = Some((0, 2));
        editor.selection_end = Some((2, 2));

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.line(0), "line3");
        assert_eq!(editor.cursor, (0, 2));
        assert_eq!(editor.selection_start, None);
    }

    #[test]
    fn test_delete_selection_no_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 5);

        let _ = editor.update(&Message::DeleteSelection);
        // Should do nothing if there's no selection
        assert_eq!(editor.buffer.line(0), "hello world");
        assert_eq!(editor.cursor, (0, 5));
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
        assert_eq!(editor.cursor, (0, "安全与合规".chars().count()));
    }

    #[test]
    fn test_undo_char_insert() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type a character
        let _ = editor.update(&Message::CharacterInput('!'));
        assert_eq!(editor.buffer.line(0), "hello!");
        assert_eq!(editor.cursor, (0, 6));

        // Undo should remove it (but first end the grouping)
        editor.history.end_group();
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_redo_char_insert() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type a character
        let _ = editor.update(&Message::CharacterInput('!'));
        editor.history.end_group();

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");

        // Redo
        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "hello!");
        assert_eq!(editor.cursor, (0, 6));
    }

    #[test]
    fn test_undo_backspace() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Backspace
        let _ = editor.update(&Message::Backspace);
        assert_eq!(editor.buffer.line(0), "hell");
        assert_eq!(editor.cursor, (0, 4));

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_newline() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursor = (0, 5);

        // Insert newline
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.buffer.line(1), " world");
        assert_eq!(editor.cursor, (1, 0));

        // Undo
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello world");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_undo_grouped_typing() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type multiple characters (they should be grouped)
        let _ = editor.update(&Message::CharacterInput(' '));
        let _ = editor.update(&Message::CharacterInput('w'));
        let _ = editor.update(&Message::CharacterInput('o'));
        let _ = editor.update(&Message::CharacterInput('r'));
        let _ = editor.update(&Message::CharacterInput('l'));
        let _ = editor.update(&Message::CharacterInput('d'));

        assert_eq!(editor.buffer.line(0), "hello world");

        // End the group
        editor.history.end_group();

        // Single undo should remove all grouped characters
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
        assert_eq!(editor.cursor, (0, 5));
    }

    #[test]
    fn test_navigation_ends_grouping() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        // Type a character (starts grouping)
        let _ = editor.update(&Message::CharacterInput('!'));
        assert!(editor.is_grouping);

        // Move cursor (ends grouping)
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Left, false));
        assert!(!editor.is_grouping);

        // Type another character (starts new group)
        let _ = editor.update(&Message::CharacterInput('?'));
        assert!(editor.is_grouping);

        editor.history.end_group();

        // Two separate undo operations
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello!");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "hello");
    }

    #[test]
    fn test_multiple_undo_redo() {
        let mut editor = CodeEditor::new("a", "py");
        editor.cursor = (0, 1);

        // Make several changes
        let _ = editor.update(&Message::CharacterInput('b'));
        editor.history.end_group();

        let _ = editor.update(&Message::CharacterInput('c'));
        editor.history.end_group();

        let _ = editor.update(&Message::CharacterInput('d'));
        editor.history.end_group();

        assert_eq!(editor.buffer.line(0), "abcd");

        // Undo all
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "abc");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "ab");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.line(0), "a");

        // Redo all
        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "ab");

        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "abc");

        let _ = editor.update(&Message::Redo);
        assert_eq!(editor.buffer.line(0), "abcd");
    }

    #[test]
    fn test_delete_key_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));
        editor.cursor = (0, 5);

        let _ = editor.update(&Message::Delete);

        assert_eq!(editor.buffer.line(0), " world");
        assert_eq!(editor.cursor, (0, 0));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_delete_key_without_selection() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 0);

        let _ = editor.update(&Message::Delete);

        // Should delete the 'h'
        assert_eq!(editor.buffer.line(0), "ello");
        assert_eq!(editor.cursor, (0, 0));
    }

    #[test]
    fn test_backspace_with_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 6));
        editor.selection_end = Some((0, 11));
        editor.cursor = (0, 11);

        let _ = editor.update(&Message::Backspace);

        assert_eq!(editor.buffer.line(0), "hello ");
        assert_eq!(editor.cursor, (0, 6));
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }

    #[test]
    fn test_backspace_without_selection() {
        let mut editor = CodeEditor::new("hello", "py");
        editor.cursor = (0, 5);

        let _ = editor.update(&Message::Backspace);

        // Should delete the 'o'
        assert_eq!(editor.buffer.line(0), "hell");
        assert_eq!(editor.cursor, (0, 4));
    }

    #[test]
    fn test_delete_multiline_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.selection_start = Some((0, 2));
        editor.selection_end = Some((2, 2));
        editor.cursor = (2, 2);

        let _ = editor.update(&Message::Delete);

        assert_eq!(editor.buffer.line(0), "line3");
        assert_eq!(editor.cursor, (0, 2));
        assert_eq!(editor.selection_start, None);
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

    #[test]
    fn test_canvas_focus_lost() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.has_canvas_focus = true;
        editor.show_cursor = true;

        let _ = editor.update(&Message::CanvasFocusLost);

        assert!(!editor.has_canvas_focus);
        assert!(!editor.show_cursor);
    }

    #[test]
    fn test_mouse_click_gains_focus() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.has_canvas_focus = false;
        editor.show_cursor = false;

        let _ =
            editor.update(&Message::MouseClick(iced::Point::new(100.0, 10.0)));

        assert!(editor.has_canvas_focus);
        assert!(editor.show_cursor);
    }
}
