//! Keyboard, mouse, and IME event handling for [`CodeEditor`]'s canvas.

use iced::advanced::input_method;
use iced::widget::canvas::Action;
use iced::{Point, Rectangle, keyboard, mouse};

use crate::canvas_editor::folding;
use crate::canvas_editor::vim::VimMode;
use crate::canvas_editor::{
    ArrowDirection, CodeEditor, FOCUSED_EDITOR_ID, Message,
};

impl CodeEditor {
    /// Checks if the editor has focus (both Iced focus and internal canvas focus).
    ///
    /// # Returns
    ///
    /// `true` if the editor has both Iced focus and internal canvas focus and is not focus-locked; `false` otherwise
    pub(crate) fn has_focus(&self) -> bool {
        // Check if this editor has Iced focus
        let focused_id =
            FOCUSED_EDITOR_ID.load(std::sync::atomic::Ordering::Relaxed);
        focused_id == self.editor_id
            && self.has_canvas_focus
            && !self.focus_locked
    }

    /// Handles keyboard shortcut combinations (Ctrl+C, Ctrl+Z, etc.).
    ///
    /// This implementation includes focus chain management for Tab and Shift+Tab
    /// navigation between editors.
    ///
    /// # Arguments
    ///
    /// * `key` - The keyboard key that was pressed
    /// * `modifiers` - The keyboard modifiers (Ctrl, Shift, Alt, etc.)
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if a shortcut was matched, `None` otherwise
    fn handle_keyboard_shortcuts(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<Action<Message>> {
        // `command()` maps to Command on macOS and Control elsewhere. Keep
        // accepting Control on macOS for backwards compatibility.
        let command_pressed = modifiers.command() || modifiers.control();

        // Toggle Vim behavior without conflicting with the platform paste
        // shortcut (Ctrl/Cmd+V).
        if command_pressed
            && modifiers.alt()
            && !modifiers.shift()
            && matches!(key, keyboard::Key::Character(v) if v.as_str() == "v")
        {
            return Some(Action::publish(Message::ToggleVimMode).and_capture());
        }

        // Handle Ctrl/Cmd+S through the same host-owned save request as Vim
        // `:w`.
        if command_pressed
            && !modifiers.alt()
            && !modifiers.shift()
            && matches!(key, keyboard::Key::Character(s) if s.as_str() == "s")
        {
            return Some(
                Action::publish(Message::WriteRequested).and_capture(),
            );
        }

        // Shift+Tab: focus navigation backward (Tab alone inserts indentation)
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab))
            && modifiers.shift()
            && !self.search_state.is_open
        {
            return Some(
                Action::publish(Message::FocusNavigationShiftTab).and_capture(),
            );
        }

        // Handle Ctrl+C / Ctrl+Insert (copy)
        if (command_pressed
            && matches!(key, keyboard::Key::Character(c) if c.as_str() == "c"))
            || (modifiers.control()
                && matches!(
                    key,
                    keyboard::Key::Named(keyboard::key::Named::Insert)
                ))
        {
            return Some(Action::publish(Message::Copy).and_capture());
        }

        // Handle Ctrl/Cmd+X (cut)
        if command_pressed
            && matches!(key, keyboard::Key::Character(x) if x.as_str() == "x")
        {
            return Some(Action::publish(Message::Cut).and_capture());
        }

        // Handle Ctrl/Cmd+A (select all)
        if command_pressed
            && matches!(key, keyboard::Key::Character(a) if a.as_str() == "a")
        {
            return Some(Action::publish(Message::SelectAll).and_capture());
        }

        // Handle Ctrl/Cmd+Z (undo). Shift+Cmd+Z is redo on macOS.
        if command_pressed
            && !modifiers.shift()
            && matches!(key, keyboard::Key::Character(z) if z.as_str() == "z")
        {
            return Some(Action::publish(Message::Undo).and_capture());
        }

        // Handle Ctrl/Cmd+Y and Shift+Cmd+Z (redo)
        if command_pressed
            && (matches!(key, keyboard::Key::Character(y) if y.as_str() == "y")
                || (modifiers.shift()
                    && matches!(key, keyboard::Key::Character(z) if z.as_str() == "z")))
        {
            return Some(Action::publish(Message::Redo).and_capture());
        }

        // Vim's redo binding is Ctrl+R in Normal mode. Keep the existing
        // platform redo shortcuts above available in every editor mode.
        if self.vim_enabled
            && self.vim_state.mode() == VimMode::Normal
            && modifiers.control()
            && !modifiers.shift()
            && matches!(key, keyboard::Key::Character(r) if r.as_str() == "r")
        {
            return Some(Action::publish(Message::Redo).and_capture());
        }

        // Handle Ctrl+F (open search)
        if command_pressed
            && matches!(key, keyboard::Key::Character(f) if f.as_str() == "f")
            && self.search_replace_enabled
        {
            return Some(Action::publish(Message::OpenSearch).and_capture());
        }

        // Handle Ctrl+H (open search and replace)
        if command_pressed
            && matches!(key, keyboard::Key::Character(h) if h.as_str() == "h")
            && self.search_replace_enabled
        {
            return Some(
                Action::publish(Message::OpenSearchReplace).and_capture(),
            );
        }

        // Handle Cmd/Ctrl+G (open go-to-line input)
        if command_pressed
            && matches!(key, keyboard::Key::Character(g) if g.as_str() == "g")
        {
            return Some(Action::publish(Message::OpenGotoLine).and_capture());
        }

        // Handle Escape — close the active overlay, or collapse multi-cursor.
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
            let message = if self.goto_line_state.is_open {
                Message::CloseGotoLine
            } else if self.search_state.is_open {
                Message::CloseSearch
            } else if self.vim_enabled {
                Message::VimKey('\u{1b}')
            } else {
                Message::CloseSearch
            };
            return Some(Action::publish(message).and_capture());
        }

        // Handle Ctrl+D (select next occurrence)
        if command_pressed
            && matches!(key, keyboard::Key::Character(d) if d.as_str() == "d")
        {
            return Some(
                Action::publish(Message::SelectNextOccurrence).and_capture(),
            );
        }

        // Handle Ctrl+/ (toggle line comment).
        //
        // Match against both the base key and `modified_key` so the shortcut
        // works regardless of layout: on US/QWERTY `/` is unshifted (in `key`),
        // while on French AZERTY it is Shift+`:` and only appears in
        // `modified_key`.
        if command_pressed
            && (matches!(key, keyboard::Key::Character(c) if c.as_str() == "/")
                || matches!(modified_key, keyboard::Key::Character(c) if c.as_str() == "/"))
        {
            return Some(Action::publish(Message::ToggleComment).and_capture());
        }

        // Handle Ctrl+Alt+Up (add cursor above)
        if modifiers.control()
            && modifiers.alt()
            && matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::ArrowUp)
            )
        {
            return Some(
                Action::publish(Message::AddCursorAbove).and_capture(),
            );
        }

        // Handle Ctrl+Alt+Down (add cursor below)
        if modifiers.control()
            && modifiers.alt()
            && matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::ArrowDown)
            )
        {
            return Some(
                Action::publish(Message::AddCursorBelow).and_capture(),
            );
        }

        // Handle Alt+Up / Alt+Down (move line) and Shift+Alt+Up / Shift+Alt+Down
        // (duplicate line). Exclude Control to avoid clashing with the
        // Ctrl+Alt+Up/Down multi-cursor shortcuts above.
        if modifiers.alt() && !modifiers.control() {
            if matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::ArrowUp)
            ) {
                let message = if modifiers.shift() {
                    Message::DuplicateLineUp
                } else {
                    Message::MoveLineUp
                };
                return Some(Action::publish(message).and_capture());
            }
            if matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::ArrowDown)
            ) {
                let message = if modifiers.shift() {
                    Message::DuplicateLineDown
                } else {
                    Message::MoveLineDown
                };
                return Some(Action::publish(message).and_capture());
            }
        }

        // Handle Tab (cycle forward in search dialog if open)
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab))
            && self.search_state.is_open
        {
            if modifiers.shift() {
                // Shift+Tab: cycle backward
                return Some(
                    Action::publish(Message::SearchDialogShiftTab)
                        .and_capture(),
                );
            } else {
                // Tab: cycle forward
                return Some(
                    Action::publish(Message::SearchDialogTab).and_capture(),
                );
            }
        }

        // Handle F3 (find next) and Shift+F3 (find previous)
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::F3))
            && self.search_replace_enabled
        {
            if modifiers.shift() {
                return Some(
                    Action::publish(Message::FindPrevious).and_capture(),
                );
            } else {
                return Some(Action::publish(Message::FindNext).and_capture());
            }
        }

        // Handle Ctrl+V / Shift+Insert (paste) - read clipboard and send paste message
        if (command_pressed
            && matches!(key, keyboard::Key::Character(v) if v.as_str() == "v"))
            || (modifiers.shift()
                && matches!(
                    key,
                    keyboard::Key::Named(keyboard::key::Named::Insert)
                ))
        {
            // Return an action that requests clipboard read
            return Some(Action::publish(Message::Paste(String::new())));
        }

        // Handle Ctrl+Home (go to start of document)
        if command_pressed
            && matches!(key, keyboard::Key::Named(keyboard::key::Named::Home))
        {
            return Some(Action::publish(Message::CtrlHome).and_capture());
        }

        // Handle Ctrl+End (go to end of document)
        if command_pressed
            && matches!(key, keyboard::Key::Named(keyboard::key::Named::End))
        {
            return Some(Action::publish(Message::CtrlEnd).and_capture());
        }

        // Handle Shift+Delete (delete selection)
        if modifiers.shift()
            && matches!(key, keyboard::Key::Named(keyboard::key::Named::Delete))
        {
            return Some(
                Action::publish(Message::DeleteSelection).and_capture(),
            );
        }

        // Code folding shortcuts (only when folding is enabled).
        if self.folding_enabled {
            // Ctrl+. : toggle the fold of the block at the cursor.
            if modifiers.control()
                && matches!(key, keyboard::Key::Character(c) if c.as_str() == ".")
            {
                return Some(
                    Action::publish(Message::ToggleFoldAtCursor).and_capture(),
                );
            }

            // Ctrl+K : fold all blocks.
            if modifiers.control()
                && !modifiers.shift()
                && matches!(key, keyboard::Key::Character(c) if c.as_str() == "k")
            {
                return Some(Action::publish(Message::FoldAll).and_capture());
            }

            // Ctrl+J : unfold all blocks.
            if modifiers.control()
                && !modifiers.shift()
                && matches!(key, keyboard::Key::Character(c) if c.as_str() == "j")
            {
                return Some(Action::publish(Message::UnfoldAll).and_capture());
            }
        }

        None
    }

    fn printable_input_message(&self, ch: char) -> Message {
        if self.vim_enabled && self.vim_state.mode() != VimMode::Insert {
            Message::VimKey(ch)
        } else {
            Message::CharacterInput(ch)
        }
    }

    /// Handles character input and special navigation keys.
    ///
    /// This implementation includes focus event propagation and focus chain management
    /// for proper focus handling without mouse bounds checking.
    ///
    /// # Arguments
    ///
    /// * `key` - The keyboard key that was pressed
    /// * `modifiers` - The keyboard modifiers (Ctrl, Shift, Alt, etc.)
    /// * `text` - Optional text content from the keyboard event
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if input should be processed, `None` otherwise
    #[allow(clippy::unused_self)]
    fn handle_character_input(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
        text: Option<&str>,
    ) -> Option<Action<Message>> {
        // Early exit: Only process character input when editor has focus
        // This prevents focus stealing where characters typed in other input fields
        // appear in the editor
        if !self.has_focus() {
            return None;
        }

        // PRIORITY 1: Check if 'text' field has valid printable character
        // This handles:
        // - Numpad keys with NumLock ON (key=Named(ArrowDown), text=Some("2"))
        // - Regular typing with shift, accents, international layouts
        if let Some(text_content) = text
            && !text_content.is_empty()
            && !modifiers.control()
            && !modifiers.alt()
        {
            // Check if it's a printable character (not a control character)
            // This filters out Enter (\n), Tab (\t), Delete (U+007F), etc.
            if let Some(first_char) = text_content.chars().next()
                && !first_char.is_control()
            {
                return Some(
                    Action::publish(self.printable_input_message(first_char))
                        .and_capture(),
                );
            }
        }

        // PRIORITY 2: Handle special named keys (navigation, editing)
        // These are only processed if text didn't contain a printable character
        let message = match key {
            keyboard::Key::Named(keyboard::key::Named::Backspace)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert
                    || self.vim_state.command_line_active() =>
            {
                if self.vim_state.command_line_active() {
                    Some(Message::VimKey('\u{8}'))
                } else {
                    Some(Message::Backspace)
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Delete)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert =>
            {
                Some(Message::Delete)
            }
            keyboard::Key::Named(keyboard::key::Named::Enter)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert
                    || self.vim_state.command_line_active() =>
            {
                if self.vim_state.command_line_active() {
                    Some(Message::VimKey('\n'))
                } else {
                    Some(Message::Enter)
                }
            }
            keyboard::Key::Named(keyboard::key::Named::Tab)
                if !self.vim_enabled
                    || self.vim_state.mode() == VimMode::Insert =>
            {
                // Handle Tab for focus navigation or text insertion
                // This implements focus event propagation and focus chain management
                if modifiers.shift() {
                    // Shift+Tab: focus navigation backward through widget hierarchy
                    Some(Message::FocusNavigationShiftTab)
                } else {
                    // Regular Tab: check if search dialog is open
                    if self.search_state.is_open {
                        Some(Message::SearchDialogTab)
                    } else {
                        // Insert 4 spaces for Tab when not in search dialog
                        Some(Message::Tab)
                    }
                }
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                Some(Message::ArrowKey(ArrowDirection::Up, modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                Some(Message::ArrowKey(ArrowDirection::Down, modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                Some(Message::ArrowKey(ArrowDirection::Left, modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some(
                Message::ArrowKey(ArrowDirection::Right, modifiers.shift()),
            ),
            keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                Some(Message::PageUp)
            }
            keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                Some(Message::PageDown)
            }
            keyboard::Key::Named(keyboard::key::Named::Home) => {
                Some(Message::Home(modifiers.shift()))
            }
            keyboard::Key::Named(keyboard::key::Named::End) => {
                Some(Message::End(modifiers.shift()))
            }
            // PRIORITY 3: Fallback to extracting from 'key' if text was empty/control char
            // This handles edge cases where text field is not populated
            _ => {
                if !modifiers.control()
                    && !modifiers.alt()
                    && let keyboard::Key::Character(c) = key
                    && !c.is_empty()
                {
                    return c
                        .chars()
                        .next()
                        .map(|ch| self.printable_input_message(ch))
                        .map(|msg| Action::publish(msg).and_capture());
                }
                None
            }
        };

        message.map(|msg| Action::publish(msg).and_capture())
    }

    /// Handles keyboard events with focus event propagation through widget hierarchy.
    ///
    /// This implementation completes focus handling without mouse bounds checking
    /// and ensures proper focus chain management.
    ///
    /// # Arguments
    ///
    /// * `key` - The keyboard key that was pressed (base key, no modifiers applied)
    /// * `modified_key` - The key with all modifiers applied except Ctrl; used
    ///   for character shortcuts so they work on layouts where the glyph needs
    ///   Shift (e.g. `/` on French AZERTY)
    /// * `modifiers` - The keyboard modifiers (Ctrl, Shift, Alt, etc.)
    /// * `text` - Optional text content from the keyboard event
    /// * `bounds` - The rectangle bounds of the canvas widget (unused in this implementation)
    /// * `cursor` - The current mouse cursor position and status (unused in this implementation)
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if the event was handled, `None` otherwise
    pub(crate) fn handle_keyboard_event(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
        text: &Option<iced::advanced::graphics::core::SmolStr>,
        _bounds: Rectangle,
        _cursor: &mouse::Cursor,
    ) -> Option<Action<Message>> {
        // Early exit: Check if editor has focus and is not focus-locked
        // This prevents focus stealing where keyboard input meant for other widgets
        // is incorrectly processed by this editor during focus transitions
        if !self.has_focus() || self.focus_locked {
            return None;
        }

        // Skip if IME is active (unless Ctrl/Command is pressed)
        if self.ime_preedit.is_some()
            && !(modifiers.control() || modifiers.command())
        {
            return None;
        }

        // Try keyboard shortcuts first
        if let Some(action) =
            self.handle_keyboard_shortcuts(key, modified_key, modifiers)
        {
            return Some(action);
        }

        // Handle character input and special keys
        // Convert Option<SmolStr> to Option<&str>
        let text_str = text.as_ref().map(|s| s.as_str());
        self.handle_character_input(key, modifiers, text_str)
    }

    /// Returns the logical line of the fold header whose chevron is at `point`,
    /// if any.
    ///
    /// Returns `None` when folding is disabled, when the point is outside the
    /// fold margin, or when the targeted line is not a fold header.
    ///
    /// # Arguments
    ///
    /// * `point` - The click position in canvas coordinates
    pub(crate) fn fold_header_at_point(&self, point: Point) -> Option<usize> {
        if !self.folding_enabled {
            return None;
        }

        // The fold margin is the strip between the line-number area and the text.
        let margin_start = self.line_number_gutter_width();
        if point.x < margin_start || point.x >= self.gutter_width() {
            return None;
        }

        let visual_line_idx = (point.y / self.line_height) as usize;
        let visual_lines = self.visual_lines_cached(self.viewport_width);
        let visual_line = visual_lines.get(visual_line_idx)?;
        if !visual_line.is_first_segment() {
            return None;
        }

        folding::is_line_fold_header(&self.buffer, visual_line.logical_line)
            .then_some(visual_line.logical_line)
    }

    /// Handles mouse events (button presses, movement, releases).
    ///
    /// # Arguments
    ///
    /// * `event` - The mouse event to handle
    /// * `bounds` - The rectangle bounds of the canvas widget
    /// * `cursor` - The current mouse cursor position and status
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if the event was handled, `None` otherwise
    #[allow(clippy::unused_self)]
    pub(crate) fn handle_mouse_event(
        &self,
        event: &mouse::Event,
        bounds: Rectangle,
        cursor: &mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            mouse::Event::ButtonPressed(mouse::Button::Left) => {
                cursor.position_in(bounds).map(|position| {
                    // Clicking a fold chevron toggles the block instead of
                    // moving the caret.
                    if let Some(header) = self.fold_header_at_point(position) {
                        return Action::publish(Message::ToggleFold(header))
                            .and_capture();
                    }

                    // Check for Ctrl (or Command on macOS) + Click
                    #[cfg(target_os = "macos")]
                    let is_jump_click = self.modifiers.get().command();
                    #[cfg(not(target_os = "macos"))]
                    let is_jump_click = self.modifiers.get().control();

                    if is_jump_click {
                        return Action::publish(Message::JumpClick(position));
                    }

                    // Alt+Click: add a new cursor at the clicked position
                    if self.modifiers.get().alt() {
                        let message = if self.vim_enabled {
                            Message::MouseClick(position)
                        } else {
                            Message::AltClick(position)
                        };
                        return Action::publish(message).and_capture();
                    }

                    let click_count = self.classify_click(position);
                    match click_count {
                        2 => Action::publish(Message::DoubleClick(position))
                            .and_capture(),
                        3 => Action::publish(Message::TripleClick(position))
                            .and_capture(),
                        // Don't capture the event so it can bubble up for focus management
                        // This implements focus event propagation through the widget hierarchy
                        _ => Action::publish(Message::MouseClick(position)),
                    }
                })
            }
            mouse::Event::ButtonPressed(mouse::Button::Right) => {
                cursor.position_in(bounds).map(|position| {
                    Action::publish(Message::ContextMenuRequested(position))
                        .and_capture()
                })
            }
            mouse::Event::CursorMoved { .. } => {
                cursor.position_in(bounds).map(|position| {
                    if self.is_dragging {
                        // Handle mouse drag for selection only when cursor is within bounds
                        Action::publish(Message::MouseDrag(position))
                            .and_capture()
                    } else {
                        // Forward hover events when not dragging to enable LSP hover.
                        Action::publish(Message::MouseHover(position))
                    }
                })
            }
            mouse::Event::ButtonReleased(mouse::Button::Left) => {
                // Only handle mouse release when cursor is within bounds
                // This prevents capturing events meant for other widgets
                if cursor.is_over(bounds) {
                    Some(Action::publish(Message::MouseRelease).and_capture())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Handles IME (Input Method Editor) events for complex text input.
    ///
    /// # Arguments
    ///
    /// * `event` - The IME event to handle
    /// * `bounds` - The rectangle bounds of the canvas widget
    /// * `cursor` - The current mouse cursor position and status
    ///
    /// # Returns
    ///
    /// `Some(Action<Message>)` if the event was handled, `None` otherwise
    pub(crate) fn handle_ime_event(
        &self,
        event: &input_method::Event,
        _bounds: Rectangle,
        _cursor: &mouse::Cursor,
    ) -> Option<Action<Message>> {
        // Early exit: Check if editor has focus and is not focus-locked
        // This prevents focus stealing where IME events meant for other widgets
        // are incorrectly processed by this editor during focus transitions
        if !self.has_focus() || self.focus_locked {
            return None;
        }
        if self.vim_enabled && self.vim_state.mode() != VimMode::Insert {
            return None;
        }

        // IME event handling
        // ---------------------------------------------------------------------
        // Core mapping: convert Iced IME events into editor Messages
        //
        // Flow:
        // 1. Opened: IME activated (e.g. switching input method). Clear old preedit state.
        // 2. Preedit: User is composing (e.g. typing "nihao" before commit).
        //    - content: current candidate text
        //    - selection: selection range within the text, in bytes
        // 3. Commit: User confirms a candidate and commits text into the buffer.
        // 4. Closed: IME closed or lost focus.
        //
        // Safety checks:
        // - handle only when `focused_id` matches this editor ID
        // - handle only when `has_canvas_focus` is true
        // This ensures IME events are not delivered to the wrong widget.
        // ---------------------------------------------------------------------
        let message = match event {
            input_method::Event::Opened => Message::ImeOpened,
            input_method::Event::Preedit(content, selection) => {
                Message::ImePreedit(content.clone(), selection.clone())
            }
            input_method::Event::Commit(content) => {
                Message::ImeCommit(content.clone())
            }
            input_method::Event::Closed => Message::ImeClosed,
        };

        Some(Action::publish(message).and_capture())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vim_navigation_keyboard_route_uses_dedicated_message() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);

        assert!(matches!(
            editor.printable_input_message('l'),
            Message::VimKey('l')
        ));

        let _ = editor.vim_state.parse_key('i');
        assert!(matches!(
            editor.printable_input_message('x'),
            Message::CharacterInput('x')
        ));

        editor.set_vim_enabled(false);
        assert!(matches!(
            editor.printable_input_message('x'),
            Message::CharacterInput('x')
        ));

        editor.set_vim_enabled(true);
        let key = keyboard::Key::Character("r".into());
        let message = editor
            .handle_keyboard_shortcuts(&key, &key, &keyboard::Modifiers::CTRL)
            .map(|action| action.into_inner().0);
        assert!(matches!(message, Some(Some(Message::Redo))));
    }

    #[test]
    fn test_vim_command_line_routes_enter_and_backspace() {
        let mut editor = CodeEditor::new("abc", "txt").with_vim_enabled(true);
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        let _ = editor.vim_state.parse_key('/');

        let backspace = editor
            .handle_character_input(
                &keyboard::Key::Named(keyboard::key::Named::Backspace),
                &keyboard::Modifiers::NONE,
                None,
            )
            .map(|action| action.into_inner().0);
        assert!(matches!(backspace, Some(Some(Message::VimKey('\u{8}')))));

        let enter = editor
            .handle_character_input(
                &keyboard::Key::Named(keyboard::key::Named::Enter),
                &keyboard::Modifiers::NONE,
                None,
            )
            .map(|action| action.into_inner().0);
        assert!(matches!(enter, Some(Some(Message::VimKey('\n')))));
    }

    #[test]
    fn test_command_g_opens_goto_line_dialog() {
        let editor = CodeEditor::new("one\ntwo", "rs");
        let key = keyboard::Key::Character("g".into());

        let message = editor
            .handle_keyboard_shortcuts(
                &key,
                &key,
                &keyboard::Modifiers::COMMAND,
            )
            .map(|action| action.into_inner().0);

        assert!(matches!(message, Some(Some(Message::OpenGotoLine))));
    }
}
