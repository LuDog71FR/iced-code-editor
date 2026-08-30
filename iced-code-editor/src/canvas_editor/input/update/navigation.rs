//! Arrow-key, Home/End, Ctrl+Home/End, Page Up/Down, and goto-position message handlers.

use iced::Task;

use crate::canvas_editor::{ArrowDirection, CodeEditor, Message, VimMode};

impl CodeEditor {
    pub(crate) fn vim_accepts_insert_input(&self) -> bool {
        !self.vim_enabled || self.vim_state.mode() == VimMode::Insert
    }

    /// Opens a navigation move: closes the current undo group and puts every
    /// cursor's selection into the state the move needs.
    ///
    /// Navigating ends the run of typing that preceded it, so the next
    /// character typed starts an undo group of its own and one Ctrl+Z does not
    /// swallow both runs. Every navigation handler needs that, which is why it
    /// lives here rather than being repeated at each call site.
    ///
    /// When Shift is held, each cursor that has no anchor yet gets one at its
    /// current position so the upcoming move extends a selection. Otherwise
    /// every selection is dropped, so the move only relocates the cursors.
    ///
    /// Pairs with [`Self::finish_navigation_operation`], which every handler
    /// calls once the move is done.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    fn begin_navigation(&mut self, shift_pressed: bool) {
        self.end_grouping_if_active();

        if shift_pressed {
            // Set anchor on ALL cursors that don't yet have one
            for cursor in self.cursors.as_mut_slice() {
                if cursor.anchor.is_none() {
                    cursor.set_anchor();
                }
            }
        } else {
            // Clear all selections so the move does not drag one along
            self.clear_selection();
        }
    }

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
    pub(crate) fn handle_arrow_key(
        &mut self,
        direction: ArrowDirection,
        shift_pressed: bool,
    ) -> Task<Message> {
        self.begin_navigation(shift_pressed);
        self.move_cursor(direction);
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
    /// A `Task<Message>` that scrolls to keep the cursor visible (including
    /// horizontal scroll back to x=0 when wrap is disabled)
    pub(crate) fn handle_home(&mut self, shift_pressed: bool) -> Task<Message> {
        self.begin_navigation(shift_pressed);
        for cursor in self.cursors.as_mut_slice() {
            cursor.position.1 = 0;
        }
        self.cursors.sort_and_merge();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
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
    /// A `Task<Message>` that scrolls to keep the cursor visible (including
    /// horizontal scroll to end of line when wrap is disabled)
    pub(crate) fn handle_end(&mut self, shift_pressed: bool) -> Task<Message> {
        self.begin_navigation(shift_pressed);
        for cursor in self.cursors.as_mut_slice() {
            cursor.position.1 = self.buffer.line_len(cursor.position.0);
        }
        self.cursors.sort_and_merge();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Ctrl+Home key press.
    ///
    /// Moves the cursor to the beginning of the document.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_ctrl_home(&mut self) -> Task<Message> {
        // Move cursor to the beginning of the document
        self.begin_navigation(false);
        self.cursors.set_single((0, 0));
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
    pub(crate) fn handle_ctrl_end(&mut self) -> Task<Message> {
        // Move cursor to the end of the document
        self.begin_navigation(false);
        let last_line = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line_len(last_line);
        self.cursors.set_single((last_line, last_col));
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Page Up key press.
    ///
    /// Moves every cursor up by one viewport height.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_page_up(
        &mut self,
        shift_pressed: bool,
    ) -> Task<Message> {
        self.begin_navigation(shift_pressed);
        self.page_up();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles Page Down key press.
    ///
    /// Moves every cursor down by one viewport height.
    ///
    /// # Arguments
    ///
    /// * `shift_pressed` - Whether Shift is held (for selection)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_page_down(
        &mut self,
        shift_pressed: bool,
    ) -> Task<Message> {
        self.begin_navigation(shift_pressed);
        self.page_down();
        self.finish_navigation_operation();
        self.scroll_to_cursor()
    }

    /// Handles direct navigation to an explicit logical position.
    ///
    /// # Arguments
    ///
    /// * `line` - Target line index (0-based)
    /// * `col` - Target column index (0-based)
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that scrolls to keep the cursor visible
    pub(crate) fn handle_goto_position(
        &mut self,
        line: usize,
        col: usize,
    ) -> Task<Message> {
        // End grouping on navigation command
        self.end_grouping_if_active();
        self.set_cursor(line, col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_home_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 5); // Move to middle of line
        let _ = editor.update(&Message::Home(false));
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_end_key() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 0);
        let _ = editor.update(&Message::End(false));
        assert_eq!(editor.cursors.primary_position(), (0, 11)); // Length of "hello world"
    }

    #[test]
    fn test_arrow_key_with_shift_creates_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().position = (0, 0);

        // Shift+Right should start selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, true));
        assert!(editor.cursors.primary().anchor.is_some());
        assert!(editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_arrow_key_without_shift_clears_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 5);

        // Regular arrow key should clear selection
        let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, false));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    /// Builds a multi-line editor whose viewport is exactly three lines tall,
    /// so `Page Up`/`Page Down` move by a known number of lines.
    fn paged_editor() -> CodeEditor {
        let content = (0..10)
            .map(|index| format!("line{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "py");
        editor.viewport_height = editor.line_height * 3.0;
        editor
    }

    #[test]
    fn test_page_down_without_shift_clears_selection() {
        let mut editor = paged_editor();
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (0, 2);

        let _ = editor.update(&Message::PageDown(false));

        assert_eq!(editor.cursors.primary_position(), (3, 2));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_page_up_without_shift_clears_selection() {
        let mut editor = paged_editor();
        editor.cursors.primary_mut().anchor = Some((9, 0));
        editor.cursors.primary_mut().position = (5, 2);

        let _ = editor.update(&Message::PageUp(false));

        assert_eq!(editor.cursors.primary_position(), (2, 2));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_page_down_with_shift_creates_selection() {
        let mut editor = paged_editor();
        editor.cursors.primary_mut().position = (0, 0);

        let _ = editor.update(&Message::PageDown(true));

        assert_eq!(editor.cursors.primary_position(), (3, 0));
        assert_eq!(editor.cursors.primary().anchor, Some((0, 0)));
        assert!(editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_page_up_with_shift_creates_selection() {
        let mut editor = paged_editor();
        editor.cursors.primary_mut().position = (5, 0);

        let _ = editor.update(&Message::PageUp(true));

        assert_eq!(editor.cursors.primary_position(), (2, 0));
        assert_eq!(editor.cursors.primary().anchor, Some((5, 0)));
        assert!(editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_page_down_after_click_does_not_select() {
        // Regression test: a plain click leaves an anchor behind for a
        // potential drag selection; Page Down must not turn it into one.
        let mut editor = paged_editor();
        let point = iced::Point::new(
            editor.gutter_width() + 5.0 + editor.char_width * 2.0,
            editor.line_height / 2.0,
        );

        let _ = editor.update(&Message::MouseClick(point));
        let _ = editor.update(&Message::MouseRelease);
        assert!(!editor.cursors.primary().has_selection());

        let _ = editor.update(&Message::PageDown(false));

        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_shift_page_down_extends_every_cursor_that_stays_apart() {
        // `page_down` loops over every cursor and then merges; the six tests
        // above all use one. Cursors far enough apart that their selections
        // never touch must each keep their own anchor.
        let mut editor = paged_editor();
        editor.cursors.set_single((0, 0));
        editor.cursors.add_cursor((8, 0));

        let _ = editor.update(&Message::PageDown(true));

        let cursors: Vec<_> = editor
            .cursors
            .as_slice()
            .iter()
            .map(|cursor| (cursor.anchor, cursor.position))
            .collect();
        assert_eq!(
            cursors,
            vec![(Some((0, 0)), (3, 0)), (Some((8, 0)), (9, 0))]
        );
    }

    #[test]
    fn test_shift_page_down_merges_selections_that_overlap() {
        // Three cursors one line apart, each extending three lines down, cover
        // 0..3, 1..4 and 2..5 -- overlapping, so `sort_and_merge` unions them
        // into the single selection 0..5. Surprising enough (three carets go
        // in, one comes out) to be worth recording as a decision rather than
        // rediscovered as a behaviour.
        let mut editor = paged_editor();
        editor.cursors.set_single((0, 0));
        editor.cursors.add_cursor((1, 0));
        editor.cursors.add_cursor((2, 0));

        let _ = editor.update(&Message::PageDown(true));

        assert_eq!(editor.cursors.len(), 1);
        assert_eq!(editor.cursors.primary().anchor, Some((0, 0)));
        assert_eq!(editor.cursors.primary_position(), (5, 0));
    }

    #[test]
    fn test_page_down_without_shift_keeps_the_cursors_it_does_not_merge() {
        // Without Shift there are no selections to overlap, so three cursors
        // three lines apart stay three cursors.
        let mut editor = paged_editor();
        editor.cursors.set_single((0, 0));
        editor.cursors.add_cursor((3, 0));
        editor.cursors.add_cursor((6, 0));

        let _ = editor.update(&Message::PageDown(false));

        let positions: Vec<_> = editor
            .cursors
            .as_slice()
            .iter()
            .map(|cursor| cursor.position)
            .collect();
        assert_eq!(positions, vec![(3, 0), (6, 0), (9, 0)]);
    }

    /// Builds a focused editor whose cursor sits mid-line, ready to type.
    fn typing_editor() -> CodeEditor {
        let mut editor = CodeEditor::new("line0\nline1\nline2", "py");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (1, 2);
        editor
    }

    #[test]
    fn test_one_undo_after_navigating_keeps_the_earlier_typing() {
        // Regression test, and the one that says what the user experiences:
        // Home, End, Ctrl+Home and Ctrl+End used to leave the undo group open,
        // so a run of typing, a navigation and a second run all collapsed into
        // one group and a single Ctrl+Z erased both runs.
        for message in [
            Message::Home(false),
            Message::End(false),
            Message::CtrlHome,
            Message::CtrlEnd,
            Message::ArrowKey(ArrowDirection::Down, false),
            Message::PageDown(false),
            Message::PageUp(false),
        ] {
            let mut editor = typing_editor();

            let _ = editor.update(&Message::CharacterInput('A'));
            let _ = editor.update(&message);
            // What one undo has to restore: the first run of typing is kept,
            // the second is not.
            let after_move = editor.content();

            let _ = editor.update(&Message::CharacterInput('B'));
            assert_ne!(editor.content(), after_move);

            let _ = editor.update(&Message::Undo);

            assert_eq!(
                editor.content(),
                after_move,
                "{message:?}: one undo did not stop at the navigation"
            );
        }
    }

    #[test]
    fn test_navigating_closes_the_undo_group() {
        for message in [
            Message::Home(false),
            Message::End(false),
            Message::CtrlHome,
            Message::CtrlEnd,
            Message::ArrowKey(ArrowDirection::Up, false),
            Message::PageDown(false),
            Message::PageUp(false),
        ] {
            let mut editor = typing_editor();

            let _ = editor.update(&Message::CharacterInput('!'));
            assert!(editor.is_grouping, "{message:?}: nothing to close");

            let _ = editor.update(&message);
            assert!(
                !editor.is_grouping,
                "{message:?} left the undo group open"
            );
        }
    }

    #[test]
    fn test_ctrl_home_still_drops_the_selection() {
        // `begin_navigation(false)` replaced a bare `clear_selection` here;
        // the selection must still go.
        let mut editor = typing_editor();
        editor.cursors.primary_mut().anchor = Some((0, 0));
        assert!(editor.cursors.primary().has_selection());

        let _ = editor.update(&Message::CtrlHome);

        assert!(!editor.cursors.primary().has_selection());
        assert_eq!(editor.cursors.primary_position(), (0, 0));
    }

    #[test]
    fn test_ctrl_home() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (2, 5); // Start at line 3, column 5
        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursors.primary_position(), (0, 0)); // Should move to beginning of document
    }

    #[test]
    fn test_ctrl_end() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (0, 0); // Start at beginning
        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursors.primary_position(), (2, 5)); // Should move to end of last line (line3 has 5 chars)
    }

    #[test]
    fn test_ctrl_home_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (2, 5);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (2, 5);

        let _ = editor.update(&Message::CtrlHome);
        assert_eq!(editor.cursors.primary_position(), (0, 0));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_ctrl_end_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().position = (0, 0);
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (1, 3);

        let _ = editor.update(&Message::CtrlEnd);
        assert_eq!(editor.cursors.primary_position(), (2, 5));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_goto_position_sets_cursor_and_clears_selection() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.cursors.primary_mut().anchor = Some((0, 0));
        editor.cursors.primary_mut().position = (1, 2);

        let _ = editor.update(&Message::GotoPosition(1, 3));

        assert_eq!(editor.cursors.primary_position(), (1, 3));
        assert!(editor.cursors.primary().anchor.is_none());
        assert!(!editor.cursors.primary().has_selection());
    }

    #[test]
    fn test_goto_position_clamps_out_of_range() {
        let mut editor = CodeEditor::new("a\nbb", "py");

        let _ = editor.update(&Message::GotoPosition(99, 99));

        // Clamped to last line (index 1) and end of that line (len = 2)
        assert_eq!(editor.cursors.primary_position(), (1, 2));
    }

    #[test]
    fn test_navigation_ends_grouping() {
        let mut editor = CodeEditor::new("hello", "py");
        // Ensure editor has focus for character input
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;

        editor.cursors.primary_mut().position = (0, 5);

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
}
