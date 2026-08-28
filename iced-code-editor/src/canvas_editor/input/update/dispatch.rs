//! Top-level message dispatcher: routes every [`Message`] variant to its handler.

use iced::Task;

use crate::canvas_editor::{CodeEditor, Message};

impl CodeEditor {
    /// Updates the editor state based on messages and returns scroll commands.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to process for updating the editor state
    ///
    /// # Returns
    /// A `Task<Message>` for any asynchronous operations, such as scrolling to keep the cursor visible after state updates
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    ///
    /// // Forward the messages the editor emits, and send your own to drive
    /// // commands from a menu or toolbar.
    /// let _task = editor.update(&Message::Paste(" world".to_string()));
    /// assert_eq!(editor.content(), " worldhello");
    ///
    /// let _task = editor.update(&Message::Undo);
    /// assert_eq!(editor.content(), "hello");
    /// ```
    pub fn update(&mut self, message: &Message) -> Task<Message> {
        // Capture the topmost active line before any edit mutates the buffer,
        // so `finish_edit_operation` can truncate the highlight cache precisely.
        self.pre_edit_line = self.min_active_line();
        self.pre_edit_last_line = self.max_active_line();
        self.capture_lsp_edit_snapshot(message);
        match message {
            // Text input operations
            Message::CharacterInput(ch) if self.vim_accepts_insert_input() => {
                self.handle_character_input_msg(*ch)
            }
            Message::CharacterInput(_) => Task::none(),
            Message::VimKey(ch) => self.handle_vim_key_msg(*ch),
            Message::ToggleVimMode => {
                self.set_vim_enabled(!self.vim_enabled);
                Task::none()
            }
            Message::Tab if self.vim_accepts_insert_input() => {
                self.handle_tab()
            }
            Message::Enter if self.vim_accepts_insert_input() => {
                self.handle_enter()
            }
            Message::Tab | Message::Enter => Task::none(),

            // Deletion operations
            Message::Backspace if self.vim_accepts_insert_input() => {
                self.handle_backspace()
            }
            Message::Delete if self.vim_accepts_insert_input() => {
                self.handle_delete()
            }
            Message::Backspace | Message::Delete => Task::none(),
            Message::DeleteSelection => self.handle_delete_selection(),

            // Navigation operations
            Message::ArrowKey(direction, shift) => {
                self.handle_arrow_key(*direction, *shift)
            }
            Message::Home(shift) => self.handle_home(*shift),
            Message::End(shift) => self.handle_end(*shift),
            Message::CtrlHome => self.handle_ctrl_home(),
            Message::CtrlEnd => self.handle_ctrl_end(),
            Message::GotoPosition(line, col) => {
                self.handle_goto_position(*line, *col)
            }
            Message::OpenGotoLine => self.handle_open_goto_line_msg(),
            Message::CloseGotoLine => self.handle_close_goto_line_msg(),
            Message::GotoLineChanged(query) => {
                self.handle_goto_line_changed_msg(query)
            }
            Message::SubmitGotoLine => self.handle_submit_goto_line_msg(),
            Message::OpenCommandPalette => {
                self.handle_open_command_palette_msg()
            }
            Message::CloseCommandPalette => {
                self.handle_close_command_palette_msg()
            }
            Message::CommandPaletteChanged(query) => {
                self.handle_command_palette_changed_msg(query)
            }
            Message::CommandPaletteNavigate(forward) => {
                self.handle_command_palette_navigate_msg(*forward)
            }
            Message::CommandPaletteSelected(index) => {
                self.handle_command_palette_selected_msg(*index)
            }
            Message::SubmitCommandPalette => {
                self.handle_submit_command_palette_msg()
            }
            Message::PageUp => self.handle_page_up(),
            Message::PageDown => self.handle_page_down(),

            // Mouse and selection operations
            Message::MouseClick(point) => self.handle_mouse_click_msg(*point),
            Message::MouseDrag(point) => self.handle_mouse_drag_msg(*point),
            Message::MouseHover(point) => self.handle_mouse_drag_msg(*point),
            Message::MouseRelease => self.handle_mouse_release_msg(),
            Message::DoubleClick(point) => self.handle_double_click_msg(*point),
            Message::TripleClick(point) => self.handle_triple_click_msg(*point),
            Message::ContextMenuRequested(point) => {
                self.handle_context_menu_requested_msg(*point)
            }
            Message::WriteRequested
            | Message::CustomContextMenuAction(_)
            | Message::CommandPaletteAction(_)
            | Message::RevealInFileManager => Task::none(),

            // Clipboard operations
            Message::Cut => self.handle_cut_msg(),
            Message::Copy => self.copy_selection(),
            Message::Paste(text) => self.handle_paste_msg(text),
            Message::SelectAll => self.handle_select_all_msg(),

            // History operations
            Message::Undo => self.handle_undo_msg(),
            Message::Redo => self.handle_redo_msg(),

            // Search and replace operations
            Message::OpenSearch => self.handle_open_search(false),
            Message::OpenSearchReplace => self.handle_open_search(true),
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
            Message::FindNext => self.handle_find_match(true),
            Message::FindPrevious => self.handle_find_match(false),
            Message::ReplaceNext => self.handle_replace_next_msg(),
            Message::ReplaceAll => self.handle_replace_all_msg(),
            Message::SearchDialogTab => self.handle_search_dialog_tab(true),
            Message::SearchDialogShiftTab => {
                self.handle_search_dialog_tab(false)
            }
            Message::FocusNavigationShiftTab => self.handle_focus_navigation(),

            // Focus and IME operations
            Message::CanvasFocusGained => self.handle_canvas_focus_gained_msg(),
            Message::CanvasFocusLost => self.handle_canvas_focus_lost_msg(),
            Message::ImeOpened if self.vim_accepts_insert_input() => {
                self.handle_ime_opened_msg()
            }
            Message::ImeOpened => Task::none(),
            Message::ImePreedit(content, selection) => {
                if self.vim_accepts_insert_input() {
                    self.handle_ime_preedit_msg(content, selection)
                } else {
                    Task::none()
                }
            }
            Message::ImeCommit(text) => self.handle_ime_commit_msg(text),
            Message::ImeClosed => self.handle_ime_closed_msg(),

            // UI update operations
            Message::Tick => self.handle_tick_msg(),
            Message::Scrolled(viewport) => self.handle_scrolled_msg(*viewport),
            Message::HorizontalScrolled(viewport) => {
                self.handle_horizontal_scrolled_msg(*viewport)
            }

            // Handle the "Jump to Definition" action triggered by Ctrl+Click.
            // Currently, this returns `Task::none()` as the actual navigation logic
            // is delegated to the `LspClient` implementation or handled elsewhere.
            Message::JumpClick(_point) => Task::none(),

            // Multi-cursor operations
            Message::AltClick(point) => self.handle_alt_click_msg(*point),
            Message::AddCursorAbove => self.handle_add_cursor_above_msg(),
            Message::AddCursorBelow => self.handle_add_cursor_below_msg(),
            Message::SelectNextOccurrence => {
                self.handle_select_next_occurrence_msg()
            }
            Message::ToggleFold(header_line) => {
                self.toggle_fold(*header_line);
                Task::none()
            }
            Message::ToggleFoldAtCursor => {
                self.toggle_fold_at(self.cursors.primary_position().0);
                Task::none()
            }
            Message::FoldAll => {
                self.fold_all();
                Task::none()
            }
            Message::UnfoldAll => {
                self.unfold_all();
                Task::none()
            }

            // Line manipulation operations
            Message::MoveLineUp => self.move_lines(false),
            Message::MoveLineDown => self.move_lines(true),
            Message::DuplicateLineUp => self.duplicate_lines(false),
            Message::DuplicateLineDown => self.duplicate_lines(true),
            Message::ToggleComment => self.toggle_comment(),

            // Sticky scroll
            Message::StickyScrollJump(line) => self.scroll_to_line(*line),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_collapses_multi_cursor() {
        let mut editor = CodeEditor::new("line0\nline1", "rs");
        editor.cursors.primary_mut().position = (0, 0);
        editor.cursors.add_cursor((1, 0));
        assert!(editor.cursors.is_multi());

        let _ = editor.update(&Message::CloseSearch);

        assert!(!editor.cursors.is_multi());
    }
}
