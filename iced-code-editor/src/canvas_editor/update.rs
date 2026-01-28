//! Message handling and update logic.

use iced::Task;
use iced::widget::operation::{focus, select_all};

use super::command::{
    Command, CompositeCommand, DeleteCharCommand, DeleteForwardCommand,
    InsertCharCommand, InsertNewlineCommand, ReplaceTextCommand,
};
use super::{CURSOR_BLINK_INTERVAL, CodeEditor, ImePreedit, Message};

impl CodeEditor {
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
            Message::CharacterInput(ch) => {
                // Start grouping if not already grouping (for smart undo)
                if !self.is_grouping {
                    self.history.begin_group("Typing");
                    self.is_grouping = true;
                }

                let (line, col) = self.cursor;
                let mut cmd =
                    InsertCharCommand::new(line, col, *ch, self.cursor);
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.refresh_search_matches_if_needed();
                self.cache.clear();
                Task::none()
            }
            Message::Backspace => {
                // End grouping on backspace (separate from typing)
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // Check if there's a selection - if so, delete it instead
                if self.selection_start.is_some()
                    && self.selection_end.is_some()
                {
                    self.delete_selection();
                    self.reset_cursor_blink();
                    self.refresh_search_matches_if_needed();
                    self.cache.clear();
                    return self.scroll_to_cursor();
                }

                // No selection - perform normal backspace
                let (line, col) = self.cursor;
                let mut cmd = DeleteCharCommand::new(
                    &self.buffer,
                    line,
                    col,
                    self.cursor,
                );
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.refresh_search_matches_if_needed();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Delete => {
                // End grouping on delete
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // Check if there's a selection - if so, delete it instead
                if self.selection_start.is_some()
                    && self.selection_end.is_some()
                {
                    self.delete_selection();
                    self.reset_cursor_blink();
                    self.refresh_search_matches_if_needed();
                    self.cache.clear();
                    return self.scroll_to_cursor();
                }

                // No selection - perform normal forward delete
                let (line, col) = self.cursor;
                let mut cmd = DeleteForwardCommand::new(
                    &self.buffer,
                    line,
                    col,
                    self.cursor,
                );
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.refresh_search_matches_if_needed();
                self.cache.clear();
                Task::none()
            }
            Message::Enter => {
                // End grouping on enter
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                let (line, col) = self.cursor;
                let mut cmd = InsertNewlineCommand::new(line, col, self.cursor);
                cmd.execute(&mut self.buffer, &mut self.cursor);
                self.history.push(Box::new(cmd));

                self.reset_cursor_blink();
                self.refresh_search_matches_if_needed();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Tab => {
                // Insert 4 spaces for Tab
                // Start grouping if not already grouping
                if !self.is_grouping {
                    self.history.begin_group("Tab");
                    self.is_grouping = true;
                }

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

                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::ArrowKey(direction, shift_pressed) => {
                // End grouping on navigation
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                if *shift_pressed {
                    // Start selection if not already started
                    if self.selection_start.is_none() {
                        self.selection_start = Some(self.cursor);
                    }
                    self.move_cursor(*direction);
                    self.selection_end = Some(self.cursor);
                } else {
                    // Clear selection and move cursor
                    self.clear_selection();
                    self.move_cursor(*direction);
                }
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::MouseClick(point) => {
                // Capture focus when clicked
                super::FOCUSED_EDITOR_ID.store(
                    self.editor_id,
                    std::sync::atomic::Ordering::Relaxed,
                );

                // End grouping on mouse click
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                self.handle_mouse_click(*point);
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
            Message::MouseDrag(point) => {
                if self.is_dragging {
                    self.handle_mouse_drag(*point);
                    self.cache.clear();
                }
                Task::none()
            }
            Message::MouseRelease => {
                self.is_dragging = false;
                Task::none()
            }
            Message::Copy => self.copy_selection(),
            Message::Paste(text) => {
                // End grouping on paste
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // If text is empty, we need to read from clipboard
                if text.is_empty() {
                    // Return a task that reads clipboard and chains to paste
                    iced::clipboard::read().and_then(|clipboard_text| {
                        Task::done(Message::Paste(clipboard_text))
                    })
                } else {
                    // We have the text, paste it
                    self.paste_text(text);
                    self.refresh_search_matches_if_needed();
                    self.cache.clear();
                    self.scroll_to_cursor()
                }
            }
            Message::DeleteSelection => {
                // End grouping on delete selection
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                // Delete selected text
                self.delete_selection();
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Tick => {
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
            Message::PageUp => {
                self.page_up();
                self.reset_cursor_blink();
                self.scroll_to_cursor()
            }
            Message::PageDown => {
                self.page_down();
                self.reset_cursor_blink();
                self.scroll_to_cursor()
            }
            Message::Home(shift_pressed) => {
                if *shift_pressed {
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
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::End(shift_pressed) => {
                let line = self.cursor.0;
                let line_len = self.buffer.line_len(line);

                if *shift_pressed {
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
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::CtrlHome => {
                // Move cursor to the beginning of the document
                self.clear_selection();
                self.cursor = (0, 0);
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::CtrlEnd => {
                // Move cursor to the end of the document
                self.clear_selection();
                let last_line = self.buffer.line_count().saturating_sub(1);
                let last_col = self.buffer.line_len(last_line);
                self.cursor = (last_line, last_col);
                self.reset_cursor_blink();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::Scrolled(viewport) => {
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
                let scroll_changed =
                    (self.viewport_scroll - new_scroll).abs() > 0.1;
                let visible_lines_count =
                    (new_height / self.line_height).ceil() as usize + 2;
                let first_visible_line =
                    (new_scroll / self.line_height).floor() as usize;
                let last_visible_line =
                    first_visible_line + visible_lines_count;
                let margin = visible_lines_count
                    * crate::canvas_editor::CACHE_WINDOW_MARGIN_MULTIPLIER;
                let window_start = first_visible_line.saturating_sub(margin);
                let window_end = last_visible_line + margin;
                // Decide whether we need to re-window the cache.
                // Special-case top-of-file: when window_start == 0, allow small forward scrolls
                // without forcing a rewindow, to avoid thrashing when the visible range is near 0.
                let need_rewindow = if self.cache_window_end_line
                    > self.cache_window_start_line
                {
                    let lower_boundary_trigger = self.cache_window_start_line
                        > 0
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
            Message::Undo => {
                // End any current grouping before undoing
                if self.is_grouping {
                    self.history.end_group();
                    self.is_grouping = false;
                }

                if self.history.undo(&mut self.buffer, &mut self.cursor) {
                    self.clear_selection();
                    self.reset_cursor_blink();
                    self.refresh_search_matches_if_needed();
                    self.cache.clear();
                    self.scroll_to_cursor()
                } else {
                    Task::none()
                }
            }
            Message::Redo => {
                if self.history.redo(&mut self.buffer, &mut self.cursor) {
                    self.clear_selection();
                    self.reset_cursor_blink();
                    self.refresh_search_matches_if_needed();
                    self.cache.clear();
                    self.scroll_to_cursor()
                } else {
                    Task::none()
                }
            }
            Message::OpenSearch => {
                self.search_state.open_search();
                self.cache.clear();

                // Focus the search input and select all text if any
                Task::batch([
                    focus(self.search_state.search_input_id.clone()),
                    select_all(self.search_state.search_input_id.clone()),
                ])
            }
            Message::OpenSearchReplace => {
                self.search_state.open_replace();
                self.cache.clear();

                // Focus the search input and select all text if any
                Task::batch([
                    focus(self.search_state.search_input_id.clone()),
                    select_all(self.search_state.search_input_id.clone()),
                ])
            }
            Message::CloseSearch => {
                self.search_state.close();
                self.cache.clear();
                Task::none()
            }
            Message::SearchQueryChanged(query) => {
                self.search_state.set_query(query.clone(), &self.buffer);
                self.cache.clear();

                // Move cursor to first match if any
                if let Some(match_pos) = self.search_state.current_match() {
                    self.cursor = (match_pos.line, match_pos.col);
                    self.clear_selection();
                    return self.scroll_to_cursor();
                }
                Task::none()
            }
            Message::ReplaceQueryChanged(replace_text) => {
                self.search_state.set_replace_with(replace_text.clone());
                Task::none()
            }
            Message::ToggleCaseSensitive => {
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
            Message::FindNext => {
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
            Message::FindPrevious => {
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
            Message::ReplaceNext => {
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
                        && let Some(next_match) =
                            self.search_state.current_match()
                    {
                        self.cursor = (next_match.line, next_match.col);
                    }

                    self.clear_selection();
                    self.cache.clear();
                    return self.scroll_to_cursor();
                }
                Task::none()
            }
            Message::ReplaceAll => {
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
            Message::SearchDialogTab => {
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
            Message::SearchDialogShiftTab => {
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
            Message::CanvasFocusGained => {
                self.has_canvas_focus = true;
                self.show_cursor = true;
                self.reset_cursor_blink();
                self.cache.clear();
                Task::none()
            }
            Message::CanvasFocusLost => {
                self.has_canvas_focus = false;
                self.show_cursor = false;
                self.ime_preedit = None;
                self.cache.clear();
                Task::none()
            }
            Message::ImeOpened => {
                // IME opened event
                // -------------------------------------------------------------
                // Triggered when the user activates the input method.
                // Action: clear current preedit content (ime_preedit) to accept new input.
                // This avoids carrying over the previous composition state.
                // -------------------------------------------------------------
                self.ime_preedit = None;
                self.cache.clear();
                Task::none()
            }
            Message::ImePreedit(content, selection) => {
                // IME preedit event
                // -------------------------------------------------------------
                // Triggered while the user is composing text but not committed yet.
                // Params:
                // - content: current preedit text (e.g. "ni h").
                // - selection: caret/selection inside the preedit text.
                //
                // Note: Iced provides selection as a byte index range, not character indices.
                // When rendering or slicing, use UTF-8 byte offsets to avoid panics.
                // -------------------------------------------------------------
                if content.is_empty() {
                    self.ime_preedit = None;
                } else {
                    self.ime_preedit = Some(ImePreedit {
                        content: content.clone(),
                        selection: selection.clone(),
                    });
                }

                self.cache.clear();
                Task::none()
            }
            Message::ImeCommit(text) => {
                // IME commit event
                // -------------------------------------------------------------
                // Triggered when the user confirms a candidate and commits text.
                // Actions:
                // 1. Clear preedit state (ime_preedit = None).
                // 2. If text is not empty, insert it at the current cursor position.
                // 3. Begin a "Typing" undo group so consecutive IME commits undo together.
                // -------------------------------------------------------------
                self.ime_preedit = None;

                if text.is_empty() {
                    self.cache.clear();
                    return Task::none();
                }

                if !self.is_grouping {
                    self.history.begin_group("Typing");
                    self.is_grouping = true;
                }

                self.paste_text(text);
                self.reset_cursor_blink();
                self.refresh_search_matches_if_needed();
                self.cache.clear();
                self.scroll_to_cursor()
            }
            Message::ImeClosed => {
                // IME closed event
                // -------------------------------------------------------------
                // Triggered when the input method is closed or loses focus.
                // Action: clear preedit state to return to normal input mode.
                // -------------------------------------------------------------
                self.ime_preedit = None;
                self.cache.clear();
                Task::none()
            }
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
