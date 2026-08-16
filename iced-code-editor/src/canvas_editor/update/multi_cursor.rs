//! Alt+Click, add-cursor-above/below, and select-next-occurrence message handlers.

use iced::Task;

use crate::buffer::text_utils::char_to_byte_index;
use crate::canvas_editor::{CodeEditor, Message, cursor_set};

impl CodeEditor {
    /// Handles Alt+Click: adds a new cursor at the clicked position without
    /// disturbing existing cursors.
    ///
    /// # Arguments
    ///
    /// * `point` - Canvas-local position of the click
    ///
    /// # Returns
    ///
    /// `Task::none()` — no async work needed
    pub(super) fn handle_alt_click_msg(
        &mut self,
        point: iced::Point,
    ) -> Task<Message> {
        if self.vim_enabled {
            return Task::none();
        }
        if let Some(pos) = self.calculate_cursor_from_point(point) {
            self.cursors.add_cursor(pos);
            self.overlay_cache.clear();
            self.reset_cursor_blink();
        }
        Task::none()
    }

    /// Handles Ctrl+Alt+Up: adds a cursor on the line above the primary cursor,
    /// at the same column (clamped to line length).
    ///
    /// # Returns
    ///
    /// `Task::none()`
    pub(super) fn handle_add_cursor_above_msg(&mut self) -> Task<Message> {
        if self.vim_enabled {
            return Task::none();
        }
        let (line, col) = self.cursors.primary_position();
        if line == 0 {
            return Task::none();
        }
        let new_line = line - 1;
        let new_col = col.min(self.buffer.line_len(new_line));
        self.cursors.add_cursor((new_line, new_col));
        self.overlay_cache.clear();
        self.reset_cursor_blink();
        Task::none()
    }

    /// Handles Ctrl+Alt+Down: adds a cursor on the line below the primary cursor,
    /// at the same column (clamped to line length).
    ///
    /// # Returns
    ///
    /// `Task::none()`
    pub(super) fn handle_add_cursor_below_msg(&mut self) -> Task<Message> {
        if self.vim_enabled {
            return Task::none();
        }
        let (line, col) = self.cursors.primary_position();
        let last_line = self.buffer.line_count().saturating_sub(1);
        if line >= last_line {
            return Task::none();
        }
        let new_line = line + 1;
        let new_col = col.min(self.buffer.line_len(new_line));
        self.cursors.add_cursor((new_line, new_col));
        self.overlay_cache.clear();
        self.reset_cursor_blink();
        Task::none()
    }

    /// Handles Ctrl+D: selects the next occurrence of the text currently selected
    /// by the primary cursor, or the word under the primary cursor if there is no
    /// selection. A new cursor with that selection is added.
    ///
    /// # Returns
    ///
    /// `Task::none()`
    pub(super) fn handle_select_next_occurrence_msg(
        &mut self,
    ) -> Task<Message> {
        if self.vim_enabled {
            return Task::none();
        }
        // Determine the search text: selected text on primary cursor, or word under cursor
        let search_text = if let Some(text) = self.get_selected_text() {
            text
        } else {
            // Select word under primary cursor first
            let (line, col) = self.cursors.primary_position();
            let line_str = self.buffer.line(line).to_string();
            let word_start = Self::word_start_in_line(&line_str, col);
            let word_end = Self::word_end_in_line(&line_str, col);
            if word_start == word_end {
                return Task::none();
            }
            // Apply selection to primary cursor and stop: the next Ctrl+D call
            // will find the next occurrence (selection will be non-empty then).
            self.cursors.primary_mut().anchor = Some((line, word_start));
            self.cursors.primary_mut().position = (line, word_end);
            self.overlay_cache.clear();
            return Task::none();
        };

        if search_text.is_empty() {
            return Task::none();
        }

        // Find the search start position: just after the last cursor's selection end
        let search_start = self
            .cursors
            .as_slice()
            .last()
            .map(|last| {
                last.selection_range()
                    .map(|(_, end)| end)
                    .unwrap_or(last.position)
            })
            .unwrap_or((0, 0));

        // Search forward from search_start for the next occurrence
        let (start_line, start_col) = search_start;
        let line_count = self.buffer.line_count();
        let search_char_len = search_text.chars().count();

        for line_offset in 0..=line_count {
            let line_idx = (start_line + line_offset) % line_count;
            let line_str = self.buffer.line(line_idx);

            // On the first iteration, start after start_col; on wrap-around, start from 0
            let search_col = if line_offset == 0 { start_col } else { 0 };

            // Build substring from search_col onward (char-indexed)
            let prefix_bytes = char_to_byte_index(line_str, search_col);
            let haystack = &line_str[prefix_bytes..];

            // The search_text is also char-based; find it as a substring
            if let Some(byte_offset) = haystack.find(search_text.as_str()) {
                // Convert byte_offset back to char offset
                let char_start =
                    search_col + haystack[..byte_offset].chars().count();
                let char_end = char_start + search_char_len;

                // Build cursor with selection for the found occurrence
                let found_cursor = cursor_set::Cursor {
                    position: (line_idx, char_end),
                    anchor: Some((line_idx, char_start)),
                };
                self.cursors.add_cursor_with_selection(found_cursor);
                self.overlay_cache.clear();
                self.reset_cursor_blink();
                return self.scroll_to_cursor();
            }
        }

        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_cursor_char_input_different_lines() {
        let mut editor = CodeEditor::new("aaa\nbbb", "rs");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        // Place cursors at (0, 1) and (1, 1)
        editor.cursors.primary_mut().position = (0, 1);
        editor.cursors.add_cursor((1, 1));

        let _ = editor.update(&Message::CharacterInput('X'));

        // Both lines should have 'X' inserted at col 1
        assert_eq!(editor.buffer.line(0), "aXaa");
        assert_eq!(editor.buffer.line(1), "bXbb");
    }

    #[test]
    fn test_multi_cursor_char_input_same_line() {
        let mut editor = CodeEditor::new("abcd", "rs");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        // Place cursors at col 1 and col 3 (same line)
        editor.cursors.primary_mut().position = (0, 1);
        editor.cursors.add_cursor((0, 3));

        let _ = editor.update(&Message::CharacterInput('X'));

        // Process descending: col 3 first → "abcXd"; then col 1 → "aXbcXd"
        // Col 1 cursor adjustment: insert at col 3 does not affect col 1 (col 1 < 3)
        assert_eq!(editor.buffer.line(0), "aXbcXd");
    }

    #[test]
    fn test_add_cursor_above() {
        let mut editor = CodeEditor::new("line0\nline1\nline2", "rs");
        editor.cursors.primary_mut().position = (1, 3);

        let _ = editor.update(&Message::AddCursorAbove);

        assert!(editor.cursors.is_multi());
        // New cursor should be at line 0, col 3
        assert_eq!(editor.cursors.as_slice()[0].position, (0, 3));
    }

    #[test]
    fn test_add_cursor_below() {
        let mut editor = CodeEditor::new("line0\nline1\nline2", "rs");
        editor.cursors.primary_mut().position = (1, 3);

        let _ = editor.update(&Message::AddCursorBelow);

        assert!(editor.cursors.is_multi());
        // New cursor should be at line 2, col 3
        assert_eq!(
            editor
                .cursors
                .as_slice()
                .iter()
                .find(|c| c.position.0 == 2)
                .map(|c| c.position),
            Some((2, 3))
        );
    }

    #[test]
    fn test_select_next_occurrence_selects_word() {
        let mut editor = CodeEditor::new("foo bar foo", "rs");
        editor.cursors.primary_mut().position = (0, 1); // inside "foo"

        let _ = editor.update(&Message::SelectNextOccurrence);

        // Primary cursor should now have "foo" selected
        let range = editor.cursors.primary().selection_range();
        assert_eq!(range, Some(((0, 0), (0, 3))));
    }

    #[test]
    fn test_select_next_occurrence_adds_cursor_for_second_occurrence() {
        let mut editor = CodeEditor::new("foo bar foo", "rs");
        // Set up primary cursor with "foo" selected
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 3);

        let _ = editor.update(&Message::SelectNextOccurrence);

        // Should now have 2 cursors: primary at "foo" (0..3) and new at "foo" (8..11)
        assert_eq!(editor.cursors.len(), 2);
    }

    #[test]
    fn test_multi_cursor_backspace() {
        let mut editor = CodeEditor::new("abc\ndef", "rs");
        editor.cursors.primary_mut().position = (0, 2);
        editor.cursors.add_cursor((1, 2));

        let _ = editor.update(&Message::Backspace);

        assert_eq!(editor.buffer.line(0), "ac");
        assert_eq!(editor.buffer.line(1), "df");
    }

    #[test]
    fn test_multi_cursor_delete_selection_undoes_in_one_step() {
        // Regression: each cursor's deletion used to be its own undo
        // command, so one undo only restored the last cursor's selection.
        // Grouped, a single undo must restore every cursor's text.
        let mut editor = CodeEditor::new("abc\ndef", "rs");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 2);
        editor.cursors.add_cursor((1, 2));
        editor.cursors.as_mut_slice()[1].anchor = Some((1, 0));

        let _ = editor.update(&Message::DeleteSelection);
        assert_eq!(editor.buffer.to_string(), "c\nf");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "abc\ndef");
    }

    #[test]
    fn test_multi_cursor_paste_undoes_in_one_step() {
        // Regression: each cursor's insertion used to be its own undo
        // command, so one undo only removed the last cursor's paste.
        // Grouped, a single undo must remove every cursor's inserted text.
        let mut editor = CodeEditor::new("a\nb", "rs");
        editor.cursors.primary_mut().position = (0, 1);
        editor.cursors.add_cursor((1, 1));

        let _ = editor.update(&Message::Paste("X".to_string()));
        assert_eq!(editor.buffer.to_string(), "aX\nbX");

        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.buffer.to_string(), "a\nb");
    }
}
