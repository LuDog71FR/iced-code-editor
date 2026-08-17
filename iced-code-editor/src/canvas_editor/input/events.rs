//! Keyboard, mouse, and IME event handling for [`CodeEditor`]'s canvas.

use iced::advanced::input_method;
use iced::widget::canvas::Action;
use iced::{Point, Rectangle, keyboard, mouse};

use crate::canvas_editor::features::folding;
use crate::canvas_editor::features::vim::VimMode;
use crate::canvas_editor::{
    ArrowDirection, CodeEditor, FOCUSED_EDITOR_ID, Message,
};

/// Returns `true` when either `key` or `modified_key` is the character `ch`.
///
/// Symbol shortcuts must consult `modified_key`, not just `key`: on layouts
/// where the glyph needs Shift (e.g. `.` and `/` on French AZERTY), `key`
/// reports the unshifted base key and only `modified_key` carries the
/// layout-modified glyph. Letter shortcuts are unaffected either way, so
/// this is safe to use uniformly for any single-character shortcut.
///
/// Every single-character shortcut in this file goes through here, letters
/// included. Only the symbol ones strictly need it, but leaving the letters
/// on a raw `matches!` left two spellings for the same check, with the
/// layout-aware one in the minority — so the next symbol shortcut would most
/// likely be copied from a raw call site and reintroduce the AZERTY bug this
/// helper exists to prevent. With no raw form left to copy, the rule enforces
/// itself.
///
/// # Examples
///
/// ```text
/// // On AZERTY, `.` is Shift+`;`: `key` is `;`, `modified_key` is `.`.
/// assert!(is_key_char(&semicolon_key, &dot_key, "."));
/// ```
fn is_key_char(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    ch: &str,
) -> bool {
    matches!(key, keyboard::Key::Character(c) if c.as_str() == ch)
        || matches!(modified_key, keyboard::Key::Character(c) if c.as_str() == ch)
}

// =============================================================================
// Keyboard shortcut groups
// =============================================================================
//
// `handle_keyboard_shortcuts` dispatches to these in a fixed order — see its
// doc comment for why that order matters. Each group below is a pure
// function of `key`/`modified_key`/`modifiers` where possible (no `self`
// field is involved) and a private method on `CodeEditor` where the
// shortcut depends on editor state (search/goto-line open, Vim mode,
// folding enabled, multi-cursor). Splitting by whether `self` is needed
// keeps every function honest about what it depends on, and the
// `clippy::unused_self` lint (denied workspace-wide) would otherwise catch
// a method that doesn't actually need `&self`.

/// Handles `Ctrl/Cmd+Alt+V`, which toggles Vim mode.
///
/// Requires Alt so it doesn't conflict with the platform paste shortcut
/// (`Ctrl/Cmd+V`, handled by [`clipboard_shortcut`]).
fn vim_toggle_shortcut(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    let command_pressed = modifiers.command() || modifiers.control();
    if command_pressed
        && modifiers.alt()
        && !modifiers.shift()
        && is_key_char(key, modified_key, "v")
    {
        return Some(Action::publish(Message::ToggleVimMode).and_capture());
    }
    None
}

/// Handles `Ctrl/Cmd+S`, routed through the same host-owned save request as
/// Vim's `:w`.
fn write_shortcut(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    let command_pressed = modifiers.command() || modifiers.control();
    if command_pressed
        && !modifiers.alt()
        && !modifiers.shift()
        && is_key_char(key, modified_key, "s")
    {
        return Some(Action::publish(Message::WriteRequested).and_capture());
    }
    None
}

/// Handles copy (`Ctrl/Cmd+C`, `Ctrl+Insert`), cut (`Ctrl/Cmd+X`), select
/// all (`Ctrl/Cmd+A`), and paste (`Ctrl/Cmd+V`, `Shift+Insert`, read from
/// clipboard and forwarded as an empty [`Message::Paste`]).
fn clipboard_shortcut(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    let command_pressed = modifiers.command() || modifiers.control();

    if (command_pressed && is_key_char(key, modified_key, "c"))
        || (modifiers.control()
            && matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::Insert)
            ))
    {
        return Some(Action::publish(Message::Copy).and_capture());
    }

    if command_pressed && is_key_char(key, modified_key, "x") {
        return Some(Action::publish(Message::Cut).and_capture());
    }

    if command_pressed && is_key_char(key, modified_key, "a") {
        return Some(Action::publish(Message::SelectAll).and_capture());
    }

    if (command_pressed && is_key_char(key, modified_key, "v"))
        || (modifiers.shift()
            && matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::Insert)
            ))
    {
        return Some(Action::publish(Message::Paste(String::new())));
    }

    None
}

/// Handles `Ctrl/Cmd+D` (select next occurrence) and `Ctrl+Alt+Up`/`Down`
/// (add a cursor above/below the current one).
fn multi_cursor_shortcut(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    let command_pressed = modifiers.command() || modifiers.control();

    if command_pressed && is_key_char(key, modified_key, "d") {
        return Some(
            Action::publish(Message::SelectNextOccurrence).and_capture(),
        );
    }

    if modifiers.control()
        && modifiers.alt()
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::ArrowUp))
    {
        return Some(Action::publish(Message::AddCursorAbove).and_capture());
    }

    if modifiers.control()
        && modifiers.alt()
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::ArrowDown))
    {
        return Some(Action::publish(Message::AddCursorBelow).and_capture());
    }

    None
}

/// Handles direct-edit shortcuts that don't touch the clipboard: toggling
/// the line comment (`Ctrl/Cmd+/`) and deleting the current selection
/// (`Shift+Delete`).
fn editing_shortcut(
    key: &keyboard::Key,
    modified_key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    let command_pressed = modifiers.command() || modifiers.control();

    // On French AZERTY `/` is Shift+`:` and only appears in `modified_key`;
    // see `is_key_char`.
    if command_pressed && is_key_char(key, modified_key, "/") {
        return Some(Action::publish(Message::ToggleComment).and_capture());
    }

    if modifiers.shift()
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::Delete))
    {
        return Some(Action::publish(Message::DeleteSelection).and_capture());
    }

    None
}

/// Handles `Alt+Up`/`Alt+Down` (move the current line) and
/// `Shift+Alt+Up`/`Down` (duplicate it). Excludes Control to avoid
/// clashing with the `Ctrl+Alt+Up`/`Down` multi-cursor shortcuts (see
/// [`multi_cursor_shortcut`]).
fn line_move_shortcut(
    key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    if !modifiers.alt() || modifiers.control() {
        return None;
    }

    if matches!(key, keyboard::Key::Named(keyboard::key::Named::ArrowUp)) {
        let message = if modifiers.shift() {
            Message::DuplicateLineUp
        } else {
            Message::MoveLineUp
        };
        return Some(Action::publish(message).and_capture());
    }

    if matches!(key, keyboard::Key::Named(keyboard::key::Named::ArrowDown)) {
        let message = if modifiers.shift() {
            Message::DuplicateLineDown
        } else {
            Message::MoveLineDown
        };
        return Some(Action::publish(message).and_capture());
    }

    None
}

/// Handles `Ctrl/Cmd+Home` (jump to the start of the document) and
/// `Ctrl/Cmd+End` (jump to the end).
fn navigation_shortcut(
    key: &keyboard::Key,
    modifiers: &keyboard::Modifiers,
) -> Option<Action<Message>> {
    let command_pressed = modifiers.command() || modifiers.control();

    if command_pressed
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::Home))
    {
        return Some(Action::publish(Message::CtrlHome).and_capture());
    }

    if command_pressed
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::End))
    {
        return Some(Action::publish(Message::CtrlEnd).and_capture());
    }

    None
}

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
    /// Tries each shortcut group in turn — see the "Keyboard shortcut
    /// groups" section above this `impl` block — and returns the first
    /// match. The order is a real dependency, not just style: a few
    /// bindings share a physical key across groups and rely on being
    /// listed in a specific relative order (e.g. `Ctrl/Cmd+Alt+V` for the
    /// Vim toggle must be tried before the plain `Ctrl/Cmd+V` paste
    /// shortcut, or the Alt modifier would fall through to paste instead).
    /// Everywhere the groups share a key, the conflicting bindings already
    /// exclude each other via their modifier checks (documented on each
    /// group), so this list is not otherwise order-sensitive.
    ///
    /// # Arguments
    ///
    /// * `key` - The keyboard key that was pressed
    /// * `modified_key` - The key with all modifiers applied except Ctrl;
    ///   used by symbol shortcuts (see [`is_key_char`])
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
        vim_toggle_shortcut(key, modified_key, modifiers)
            .or_else(|| write_shortcut(key, modified_key, modifiers))
            .or_else(|| self.focus_navigation_shortcut(key, modifiers))
            .or_else(|| clipboard_shortcut(key, modified_key, modifiers))
            .or_else(|| self.history_shortcut(key, modified_key, modifiers))
            .or_else(|| self.dialog_shortcut(key, modified_key, modifiers))
            .or_else(|| self.escape_shortcut(key))
            .or_else(|| multi_cursor_shortcut(key, modified_key, modifiers))
            .or_else(|| editing_shortcut(key, modified_key, modifiers))
            .or_else(|| line_move_shortcut(key, modifiers))
            .or_else(|| navigation_shortcut(key, modifiers))
            .or_else(|| self.folding_shortcut(key, modified_key, modifiers))
    }

    /// Handles `Shift+Tab` for backward focus-chain navigation between
    /// editors. Plain `Tab` inserts indentation instead, handled by
    /// [`Self::handle_character_input`]. Skipped while the search dialog is
    /// open, where Tab/Shift+Tab cycle its fields instead (see
    /// [`Self::dialog_shortcut`]).
    fn focus_navigation_shortcut(
        &self,
        key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<Action<Message>> {
        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab))
            && modifiers.shift()
            && !self.search_state.is_open
        {
            return Some(
                Action::publish(Message::FocusNavigationShiftTab).and_capture(),
            );
        }
        None
    }

    /// Handles undo (`Ctrl/Cmd+Z`), redo (`Ctrl/Cmd+Y`, or `Shift+Cmd+Z` on
    /// macOS), and Vim's `Ctrl+R` redo binding in Normal mode — kept
    /// alongside the platform shortcuts above, which stay available in
    /// every editor mode, not just Vim's Normal mode.
    fn history_shortcut(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<Action<Message>> {
        let command_pressed = modifiers.command() || modifiers.control();

        if command_pressed
            && !modifiers.shift()
            && is_key_char(key, modified_key, "z")
        {
            return Some(Action::publish(Message::Undo).and_capture());
        }

        if command_pressed
            && (is_key_char(key, modified_key, "y")
                || (modifiers.shift() && is_key_char(key, modified_key, "z")))
        {
            return Some(Action::publish(Message::Redo).and_capture());
        }

        if self.vim_enabled
            && self.vim_state.mode() == VimMode::Normal
            && modifiers.control()
            && !modifiers.shift()
            && is_key_char(key, modified_key, "r")
        {
            return Some(Action::publish(Message::Redo).and_capture());
        }

        None
    }

    /// Handles the search/replace and goto-line dialogs: opening them
    /// (`Ctrl/Cmd+F`, `Ctrl/Cmd+H`, `Ctrl/Cmd+G`), cycling the search
    /// dialog's fields while it is open (`Tab`/`Shift+Tab`), and find
    /// next/previous (`F3`/`Shift+F3`).
    fn dialog_shortcut(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<Action<Message>> {
        let command_pressed = modifiers.command() || modifiers.control();

        if command_pressed
            && is_key_char(key, modified_key, "f")
            && self.search_replace_enabled
        {
            return Some(Action::publish(Message::OpenSearch).and_capture());
        }

        if command_pressed
            && is_key_char(key, modified_key, "h")
            && self.search_replace_enabled
        {
            return Some(
                Action::publish(Message::OpenSearchReplace).and_capture(),
            );
        }

        if command_pressed && is_key_char(key, modified_key, "g") {
            return Some(Action::publish(Message::OpenGotoLine).and_capture());
        }

        if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab))
            && self.search_state.is_open
        {
            let message = if modifiers.shift() {
                Message::SearchDialogShiftTab
            } else {
                Message::SearchDialogTab
            };
            return Some(Action::publish(message).and_capture());
        }

        if matches!(key, keyboard::Key::Named(keyboard::key::Named::F3))
            && self.search_replace_enabled
        {
            let message = if modifiers.shift() {
                Message::FindPrevious
            } else {
                Message::FindNext
            };
            return Some(Action::publish(message).and_capture());
        }

        None
    }

    /// Handles Escape: closes the active dialog, collapses a multi-cursor
    /// selection down to the primary cursor (`handle_close_search_msg`
    /// does this when more than one cursor is active, even with no dialog
    /// open), or forwards it to Vim's modal state machine. If none of
    /// these apply, Escape has nothing to do here, so the event is left
    /// uncaptured (`None`) instead of being swallowed on a no-op — a host
    /// embedding the editor can then react to it (e.g. closing a modal,
    /// leaving fullscreen).
    fn escape_shortcut(&self, key: &keyboard::Key) -> Option<Action<Message>> {
        if !matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
            return None;
        }

        let message = if self.goto_line_state.is_open {
            Some(Message::CloseGotoLine)
        } else if self.search_state.is_open {
            Some(Message::CloseSearch)
        } else if self.vim_enabled {
            Some(Message::VimKey('\u{1b}'))
        } else if self.cursors.is_multi() {
            Some(Message::CloseSearch)
        } else {
            None
        };
        message.map(|message| Action::publish(message).and_capture())
    }

    /// Handles code-folding shortcuts, active only while folding is
    /// enabled: toggling the fold at the cursor (`Ctrl/Cmd+.`), folding
    /// everything (`Ctrl+K`), and unfolding everything (`Ctrl+J`).
    fn folding_shortcut(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<Action<Message>> {
        if !self.folding_enabled {
            return None;
        }

        // On French AZERTY `.` is Shift+`;` and only appears in
        // `modified_key`; see `is_key_char`.
        if modifiers.control() && is_key_char(key, modified_key, ".") {
            return Some(
                Action::publish(Message::ToggleFoldAtCursor).and_capture(),
            );
        }

        // Exclude Alt so it doesn't clash with a future Ctrl+Alt+K binding,
        // matching the Ctrl+Alt+Up/Down and Alt+Up/Down shortcuts, which
        // each exclude the other modifier for the same reason.
        if modifiers.control()
            && !modifiers.shift()
            && !modifiers.alt()
            && is_key_char(key, modified_key, "k")
        {
            return Some(Action::publish(Message::FoldAll).and_capture());
        }

        if modifiers.control()
            && !modifiers.shift()
            && !modifiers.alt()
            && is_key_char(key, modified_key, "j")
        {
            return Some(Action::publish(Message::UnfoldAll).and_capture());
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
    use crate::canvas_editor::metrics::compare_floats;
    use iced::event;
    use std::cmp::Ordering;

    /// Shorthand for a printable-character key.
    fn character(ch: &str) -> keyboard::Key {
        keyboard::Key::Character(ch.into())
    }

    /// Shorthand for a named (non-printable) key.
    fn named(key: keyboard::key::Named) -> keyboard::Key {
        keyboard::Key::Named(key)
    }

    /// Runs `key` + `modifiers` through the whole shortcut chain and returns
    /// the message it publishes, if any.
    ///
    /// `modified_key` is passed as a copy of `key`, which is what a QWERTY
    /// layout reports; the AZERTY tests supply the two separately to exercise
    /// [`is_key_char`]. Going through [`CodeEditor::handle_keyboard_shortcuts`]
    /// rather than calling each group directly is deliberate: it is the group
    /// *ordering* that encodes the shared-key precedence rules, so a test that
    /// bypassed the chain would not catch a group being reordered.
    fn shortcut(
        editor: &CodeEditor,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<Message> {
        editor
            .handle_keyboard_shortcuts(key, key, &modifiers)
            .and_then(|action| action.into_inner().0)
    }

    /// Returns the event status the shortcut chain reports for `key`.
    fn shortcut_status(
        editor: &CodeEditor,
        key: &keyboard::Key,
        modifiers: keyboard::Modifiers,
    ) -> Option<event::Status> {
        editor
            .handle_keyboard_shortcuts(key, key, &modifiers)
            .map(|action| action.into_inner().2)
    }

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

    #[test]
    fn test_is_key_char_matches_base_or_modified_key() {
        let dot = keyboard::Key::Character(".".into());
        let semicolon = keyboard::Key::Character(";".into());
        let g = keyboard::Key::Character("g".into());

        // QWERTY: `.` is unshifted, so it shows up as the base key.
        assert!(is_key_char(&dot, &dot, "."));
        // AZERTY: `.` is Shift+`;`, so only `modified_key` carries it.
        assert!(is_key_char(&semicolon, &dot, "."));
        // Neither key matches.
        assert!(!is_key_char(&g, &g, "."));
    }

    #[test]
    fn test_ctrl_dot_toggles_fold_on_azerty_layout() {
        let editor = CodeEditor::new("fn a() {\n}\n", "rs");
        assert!(editor.folding_enabled);

        // AZERTY: `.` requires Shift, so `key` reports the unshifted `;`
        // and only `modified_key` reports `.`.
        let base_key = keyboard::Key::Character(";".into());
        let modified_key = keyboard::Key::Character(".".into());

        let message = editor
            .handle_keyboard_shortcuts(
                &base_key,
                &modified_key,
                &keyboard::Modifiers::CTRL,
            )
            .map(|action| action.into_inner().0);

        assert!(matches!(message, Some(Some(Message::ToggleFoldAtCursor))));
    }

    #[test]
    fn test_ctrl_slash_toggles_comment_on_azerty_layout() {
        let editor = CodeEditor::new("fn a() {}\n", "rs");

        // AZERTY: `/` requires Shift, so `key` reports the unshifted `:`
        // and only `modified_key` reports `/`.
        let base_key = keyboard::Key::Character(":".into());
        let modified_key = keyboard::Key::Character("/".into());

        let message = editor
            .handle_keyboard_shortcuts(
                &base_key,
                &modified_key,
                &keyboard::Modifiers::CTRL,
            )
            .map(|action| action.into_inner().0);

        assert!(matches!(message, Some(Some(Message::ToggleComment))));
    }

    #[test]
    fn test_every_single_character_shortcut_consults_the_modified_key() {
        let editor = CodeEditor::new("fn a() {\n}\n", "rs");
        let ctrl = keyboard::Modifiers::CTRL;
        let ctrl_alt = keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT;
        // A base key that matches no shortcut, so only `modified_key` can
        // possibly satisfy the check.
        let unmatched = character("&");

        // Every single-character binding in the chain, letters included. A
        // call site reverted to a raw `matches!` on `key` alone would stop
        // seeing the character here and fail — which is the point: only the
        // symbol shortcuts are broken on AZERTY today, so nothing else would
        // catch the letters drifting back.
        /// One row of the table: the character, the modifiers it needs, and a
        /// predicate matching the message it must publish.
        type ShortcutCase =
            (&'static str, keyboard::Modifiers, fn(&Message) -> bool);

        let cases: [ShortcutCase; 15] = [
            ("v", ctrl_alt, |m| matches!(m, Message::ToggleVimMode)),
            ("s", ctrl, |m| matches!(m, Message::WriteRequested)),
            ("c", ctrl, |m| matches!(m, Message::Copy)),
            ("x", ctrl, |m| matches!(m, Message::Cut)),
            ("a", ctrl, |m| matches!(m, Message::SelectAll)),
            ("v", ctrl, |m| matches!(m, Message::Paste(_))),
            ("d", ctrl, |m| matches!(m, Message::SelectNextOccurrence)),
            ("z", ctrl, |m| matches!(m, Message::Undo)),
            ("y", ctrl, |m| matches!(m, Message::Redo)),
            ("f", ctrl, |m| matches!(m, Message::OpenSearch)),
            ("h", ctrl, |m| matches!(m, Message::OpenSearchReplace)),
            ("g", ctrl, |m| matches!(m, Message::OpenGotoLine)),
            ("k", ctrl, |m| matches!(m, Message::FoldAll)),
            ("j", ctrl, |m| matches!(m, Message::UnfoldAll)),
            (".", ctrl, |m| matches!(m, Message::ToggleFoldAtCursor)),
        ];

        for (ch, modifiers, expected) in cases {
            let message = editor
                .handle_keyboard_shortcuts(
                    &unmatched,
                    &character(ch),
                    &modifiers,
                )
                .and_then(|action| action.into_inner().0);
            assert!(
                message.as_ref().is_some_and(expected),
                "shortcut for {ch:?} ignored `modified_key`, got {message:?}"
            );
        }
    }

    #[test]
    fn test_ctrl_alt_k_does_not_fold_all() {
        let editor = CodeEditor::new("fn a() {\n}\n", "rs");
        assert!(editor.folding_enabled);

        let key = keyboard::Key::Character("k".into());
        let modifiers = keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT;

        let message = editor
            .handle_keyboard_shortcuts(&key, &key, &modifiers)
            .map(|action| action.into_inner().0);

        // Ctrl+Alt+K must not trigger the plain Ctrl+K fold-all shortcut,
        // leaving room for a future Ctrl+Alt+K binding.
        assert!(!matches!(message, Some(Some(Message::FoldAll))));
    }

    #[test]
    fn test_escape_with_nothing_to_close_is_not_captured() {
        let editor = CodeEditor::new("abc", "txt");
        assert!(!editor.goto_line_state.is_open);
        assert!(!editor.search_state.is_open);
        assert!(!editor.vim_enabled);
        assert!(!editor.cursors.is_multi());

        let key = keyboard::Key::Named(keyboard::key::Named::Escape);
        let message = editor.handle_keyboard_shortcuts(
            &key,
            &key,
            &keyboard::Modifiers::NONE,
        );

        // Nothing for the editor to do with Escape here, so the event must
        // be left uncaptured for the host application to handle.
        assert!(message.is_none());
    }

    #[test]
    fn test_escape_with_multi_cursor_and_nothing_else_open_is_captured() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
        editor.cursors.add_cursor((1, 0));
        assert!(editor.cursors.is_multi());

        let key = keyboard::Key::Named(keyboard::key::Named::Escape);
        let message = editor
            .handle_keyboard_shortcuts(&key, &key, &keyboard::Modifiers::NONE)
            .map(|action| action.into_inner().0);

        assert!(matches!(message, Some(Some(Message::CloseSearch))));
    }

    // =========================================================================
    // Shortcut groups
    // =========================================================================
    //
    // One test per group declared above the `impl CodeEditor` block, plus the
    // cross-group precedence cases where two groups share a physical key. The
    // groups are only correct as an ordered chain, so every case below goes
    // through `handle_keyboard_shortcuts` rather than calling a group directly.

    #[test]
    fn test_clipboard_shortcuts_route_copy_cut_select_all_and_paste() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let ctrl = keyboard::Modifiers::CTRL;

        assert!(matches!(
            shortcut(&editor, &character("c"), ctrl),
            Some(Message::Copy)
        ));
        assert!(matches!(
            shortcut(&editor, &character("x"), ctrl),
            Some(Message::Cut)
        ));
        assert!(matches!(
            shortcut(&editor, &character("a"), ctrl),
            Some(Message::SelectAll)
        ));
        // Paste carries an empty string: the host reads the real clipboard
        // and re-dispatches the text.
        assert!(matches!(
            shortcut(&editor, &character("v"), ctrl),
            Some(Message::Paste(text)) if text.is_empty()
        ));
    }

    #[test]
    fn test_clipboard_shortcuts_accept_the_insert_key_bindings() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let insert = named(keyboard::key::Named::Insert);

        assert!(matches!(
            shortcut(&editor, &insert, keyboard::Modifiers::CTRL),
            Some(Message::Copy)
        ));
        assert!(matches!(
            shortcut(&editor, &insert, keyboard::Modifiers::SHIFT),
            Some(Message::Paste(_))
        ));
    }

    #[test]
    fn test_paste_is_the_only_clipboard_shortcut_left_uncaptured() {
        let editor = CodeEditor::new("one", "txt");
        let ctrl = keyboard::Modifiers::CTRL;

        for ch in ["c", "x", "a"] {
            assert!(
                matches!(
                    shortcut_status(&editor, &character(ch), ctrl),
                    Some(event::Status::Captured)
                ),
                "Ctrl+{ch} should capture the event"
            );
        }

        // Paste is published without `.and_capture()`, unlike every other
        // shortcut in the chain. Locking that in here because it is a silent
        // asymmetry: the message alone looks identical to the captured ones.
        assert!(matches!(
            shortcut_status(&editor, &character("v"), ctrl),
            Some(event::Status::Ignored)
        ));
    }

    #[test]
    fn test_multi_cursor_shortcuts_select_occurrence_and_add_cursors() {
        let editor = CodeEditor::new("one\ntwo\nthree", "txt");
        let ctrl_alt = keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT;

        assert!(matches!(
            shortcut(&editor, &character("d"), keyboard::Modifiers::CTRL),
            Some(Message::SelectNextOccurrence)
        ));
        assert!(matches!(
            shortcut(&editor, &named(keyboard::key::Named::ArrowUp), ctrl_alt),
            Some(Message::AddCursorAbove)
        ));
        assert!(matches!(
            shortcut(
                &editor,
                &named(keyboard::key::Named::ArrowDown),
                ctrl_alt
            ),
            Some(Message::AddCursorBelow)
        ));
    }

    #[test]
    fn test_shift_delete_deletes_the_selection() {
        let editor = CodeEditor::new("one\ntwo", "txt");

        assert!(matches!(
            shortcut(
                &editor,
                &named(keyboard::key::Named::Delete),
                keyboard::Modifiers::SHIFT
            ),
            Some(Message::DeleteSelection)
        ));
    }

    #[test]
    fn test_line_move_shortcuts_move_and_duplicate_lines() {
        let editor = CodeEditor::new("one\ntwo\nthree", "txt");
        let alt = keyboard::Modifiers::ALT;
        let shift_alt = keyboard::Modifiers::SHIFT | keyboard::Modifiers::ALT;
        let up = named(keyboard::key::Named::ArrowUp);
        let down = named(keyboard::key::Named::ArrowDown);

        assert!(matches!(
            shortcut(&editor, &up, alt),
            Some(Message::MoveLineUp)
        ));
        assert!(matches!(
            shortcut(&editor, &down, alt),
            Some(Message::MoveLineDown)
        ));
        assert!(matches!(
            shortcut(&editor, &up, shift_alt),
            Some(Message::DuplicateLineUp)
        ));
        assert!(matches!(
            shortcut(&editor, &down, shift_alt),
            Some(Message::DuplicateLineDown)
        ));
    }

    #[test]
    fn test_control_alt_arrows_add_cursors_instead_of_moving_lines() {
        let editor = CodeEditor::new("one\ntwo\nthree", "txt");
        let ctrl_alt = keyboard::Modifiers::CTRL | keyboard::Modifiers::ALT;

        // `line_move_shortcut` excludes Control precisely so Ctrl+Alt+Arrow
        // stays with the multi-cursor group. Same class of clash as the
        // Ctrl+Alt+K fold-all bug tested above.
        assert!(!matches!(
            shortcut(&editor, &named(keyboard::key::Named::ArrowUp), ctrl_alt),
            Some(Message::MoveLineUp | Message::DuplicateLineUp)
        ));
        assert!(!matches!(
            shortcut(
                &editor,
                &named(keyboard::key::Named::ArrowDown),
                ctrl_alt
            ),
            Some(Message::MoveLineDown | Message::DuplicateLineDown)
        ));
    }

    #[test]
    fn test_navigation_shortcuts_jump_to_document_bounds() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let ctrl = keyboard::Modifiers::CTRL;

        assert!(matches!(
            shortcut(&editor, &named(keyboard::key::Named::Home), ctrl),
            Some(Message::CtrlHome)
        ));
        assert!(matches!(
            shortcut(&editor, &named(keyboard::key::Named::End), ctrl),
            Some(Message::CtrlEnd)
        ));
    }

    #[test]
    fn test_shift_tab_navigates_focus_unless_the_search_dialog_is_open() {
        let mut editor = CodeEditor::new("one", "txt");
        let tab = named(keyboard::key::Named::Tab);
        let shift = keyboard::Modifiers::SHIFT;

        assert!(!editor.search_state.is_open);
        assert!(matches!(
            shortcut(&editor, &tab, shift),
            Some(Message::FocusNavigationShiftTab)
        ));

        // With the dialog open, Tab/Shift+Tab cycle its fields instead of
        // leaving the editor.
        editor.search_state.is_open = true;
        assert!(matches!(
            shortcut(&editor, &tab, shift),
            Some(Message::SearchDialogShiftTab)
        ));
        assert!(matches!(
            shortcut(&editor, &tab, keyboard::Modifiers::NONE),
            Some(Message::SearchDialogTab)
        ));
    }

    #[test]
    fn test_history_shortcuts_route_undo_and_both_redo_bindings() {
        let editor = CodeEditor::new("one", "txt");
        let ctrl = keyboard::Modifiers::CTRL;
        let ctrl_shift = keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT;

        assert!(matches!(
            shortcut(&editor, &character("z"), ctrl),
            Some(Message::Undo)
        ));
        assert!(matches!(
            shortcut(&editor, &character("y"), ctrl),
            Some(Message::Redo)
        ));
        // Shift+Ctrl/Cmd+Z is the macOS redo binding, accepted everywhere.
        assert!(matches!(
            shortcut(&editor, &character("z"), ctrl_shift),
            Some(Message::Redo)
        ));
    }

    #[test]
    fn test_dialog_shortcuts_open_dialogs_and_drive_find_next_previous() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let ctrl = keyboard::Modifiers::CTRL;
        let f3 = named(keyboard::key::Named::F3);

        assert!(editor.search_replace_enabled);
        assert!(matches!(
            shortcut(&editor, &character("f"), ctrl),
            Some(Message::OpenSearch)
        ));
        assert!(matches!(
            shortcut(&editor, &character("h"), ctrl),
            Some(Message::OpenSearchReplace)
        ));
        assert!(matches!(
            shortcut(&editor, &f3, keyboard::Modifiers::NONE),
            Some(Message::FindNext)
        ));
        assert!(matches!(
            shortcut(&editor, &f3, keyboard::Modifiers::SHIFT),
            Some(Message::FindPrevious)
        ));
    }

    #[test]
    fn test_search_shortcuts_are_inert_when_search_replace_is_disabled() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        editor.set_search_replace_enabled(false);
        let ctrl = keyboard::Modifiers::CTRL;

        assert!(shortcut(&editor, &character("f"), ctrl).is_none());
        assert!(shortcut(&editor, &character("h"), ctrl).is_none());
        assert!(
            shortcut(
                &editor,
                &named(keyboard::key::Named::F3),
                keyboard::Modifiers::NONE
            )
            .is_none()
        );

        // Goto-line is not gated on the search/replace setting.
        assert!(matches!(
            shortcut(&editor, &character("g"), ctrl),
            Some(Message::OpenGotoLine)
        ));
    }

    #[test]
    fn test_escape_closes_the_goto_line_dialog_before_the_search_dialog() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        let escape = named(keyboard::key::Named::Escape);
        let none = keyboard::Modifiers::NONE;

        editor.goto_line_state.is_open = true;
        assert!(matches!(
            shortcut(&editor, &escape, none),
            Some(Message::CloseGotoLine)
        ));

        // Both open: goto-line is the innermost dialog, so it closes first.
        editor.search_state.is_open = true;
        assert!(matches!(
            shortcut(&editor, &escape, none),
            Some(Message::CloseGotoLine)
        ));

        editor.goto_line_state.is_open = false;
        assert!(matches!(
            shortcut(&editor, &escape, none),
            Some(Message::CloseSearch)
        ));
    }

    #[test]
    fn test_escape_reaches_vim_only_when_no_dialog_is_open() {
        let mut editor = CodeEditor::new("one", "txt").with_vim_enabled(true);
        let escape = named(keyboard::key::Named::Escape);
        let none = keyboard::Modifiers::NONE;

        assert!(matches!(
            shortcut(&editor, &escape, none),
            Some(Message::VimKey('\u{1b}'))
        ));

        // With a dialog open, Escape closes it rather than being swallowed by
        // Vim's modal state machine — otherwise the dialog would be
        // unclosable while Vim is on.
        editor.search_state.is_open = true;
        assert!(matches!(
            shortcut(&editor, &escape, none),
            Some(Message::CloseSearch)
        ));
    }

    #[test]
    fn test_folding_shortcuts_toggle_fold_and_unfold_everything() {
        let editor = CodeEditor::new("fn a() {\n}\n", "rs");
        let ctrl = keyboard::Modifiers::CTRL;
        assert!(editor.folding_enabled);

        // QWERTY counterpart of the AZERTY test above: `.` needs no Shift, so
        // it arrives as the base key.
        assert!(matches!(
            shortcut(&editor, &character("."), ctrl),
            Some(Message::ToggleFoldAtCursor)
        ));
        assert!(matches!(
            shortcut(&editor, &character("k"), ctrl),
            Some(Message::FoldAll)
        ));
        assert!(matches!(
            shortcut(&editor, &character("j"), ctrl),
            Some(Message::UnfoldAll)
        ));
    }

    #[test]
    fn test_fold_all_and_unfold_all_exclude_shift() {
        let editor = CodeEditor::new("fn a() {\n}\n", "rs");
        let ctrl_shift = keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT;

        // Same guard as `test_ctrl_alt_k_does_not_fold_all`: the fold
        // bindings exclude Shift and Alt so those combinations stay free.
        assert!(shortcut(&editor, &character("k"), ctrl_shift).is_none());
        assert!(shortcut(&editor, &character("j"), ctrl_shift).is_none());
    }

    #[test]
    fn test_folding_shortcuts_are_inert_when_folding_is_disabled() {
        let editor =
            CodeEditor::new("fn a() {\n}\n", "rs").with_folding_enabled(false);
        let ctrl = keyboard::Modifiers::CTRL;

        assert!(!editor.folding_enabled);
        assert!(shortcut(&editor, &character("."), ctrl).is_none());
        assert!(shortcut(&editor, &character("k"), ctrl).is_none());
        assert!(shortcut(&editor, &character("j"), ctrl).is_none());
    }

    // =========================================================================
    // Mouse events
    // =========================================================================
    //
    // `handle_mouse_event` reports positions *relative to the canvas bounds*,
    // so `MOUSE_BOUNDS` is deliberately offset from the origin: with bounds at
    // (0, 0) an absolute-position lookup and a bounds-relative one are
    // indistinguishable, and every position assertion below would still pass
    // if `position_in` were swapped for `position`.

    /// Canvas bounds shared by the mouse tests, offset from the origin on
    /// purpose — see the note above.
    const MOUSE_BOUNDS: Rectangle =
        Rectangle { x: 10.0, y: 20.0, width: 400.0, height: 300.0 };

    /// A cursor sitting at `(x, y)` measured from [`MOUSE_BOUNDS`]'s top-left
    /// corner, i.e. in the same coordinate space as the published messages.
    fn cursor_at(x: f32, y: f32) -> mouse::Cursor {
        mouse::Cursor::Available(Point::new(
            MOUSE_BOUNDS.x + x,
            MOUSE_BOUNDS.y + y,
        ))
    }

    /// Feeds `event` to the editor and returns the message it publishes.
    ///
    /// Not idempotent for left presses: each one advances the editor's
    /// click-count state, so calling this twice at the same spot yields a
    /// double click. Use a fresh editor when a test needs two independent
    /// single clicks.
    fn mouse_message(
        editor: &CodeEditor,
        event: &mouse::Event,
        cursor: &mouse::Cursor,
    ) -> Option<Message> {
        editor
            .handle_mouse_event(event, MOUSE_BOUNDS, cursor)
            .and_then(|action| action.into_inner().0)
    }

    /// Returns the event status the editor reports for `event`.
    fn mouse_status(
        editor: &CodeEditor,
        event: &mouse::Event,
        cursor: &mouse::Cursor,
    ) -> Option<event::Status> {
        editor
            .handle_mouse_event(event, MOUSE_BOUNDS, cursor)
            .map(|action| action.into_inner().2)
    }

    /// Extracts the canvas position carried by the position-bearing mouse
    /// messages, so a test can assert on it without destructuring (`panic!`
    /// is denied workspace-wide, which rules out a `let ... else` binding).
    fn message_position(message: &Option<Message>) -> Option<Point> {
        match message {
            Some(
                Message::MouseClick(position)
                | Message::MouseHover(position)
                | Message::MouseDrag(position)
                | Message::AltClick(position)
                | Message::JumpClick(position)
                | Message::ContextMenuRequested(position),
            ) => Some(*position),
            _ => None,
        }
    }

    /// Asserts `message` carries the position `(x, y)`, compared within the
    /// project's float epsilon (`float_cmp` is denied workspace-wide).
    fn assert_position(message: &Option<Message>, x: f32, y: f32) {
        let position = message_position(message);
        assert!(
            position.is_some_and(|position| {
                compare_floats(position.x, x) == Ordering::Equal
                    && compare_floats(position.y, y) == Ordering::Equal
            }),
            "expected position ({x}, {y}), got {message:?}"
        );
    }

    /// A left-button press, the event most of the mouse tests start from.
    const LEFT_PRESS: mouse::Event =
        mouse::Event::ButtonPressed(mouse::Button::Left);

    #[test]
    fn test_left_click_publishes_a_bounds_relative_position() {
        let editor = CodeEditor::new("one\ntwo", "txt");

        let message =
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(30.0, 40.0));
        assert!(
            matches!(message, Some(Message::MouseClick(_))),
            "expected a MouseClick, got {message:?}"
        );
        assert_position(&message, 30.0, 40.0);

        // A plain click stays uncaptured so it can bubble up for focus
        // management — unlike every other left-click outcome below. Checked on
        // a fresh editor because left clicks accumulate click-count state.
        let editor = CodeEditor::new("one\ntwo", "txt");
        assert!(matches!(
            mouse_status(&editor, &LEFT_PRESS, &cursor_at(30.0, 40.0)),
            Some(event::Status::Ignored)
        ));
    }

    #[test]
    fn test_mouse_events_outside_the_canvas_are_left_alone() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let outside = mouse::Cursor::Available(Point::new(
            MOUSE_BOUNDS.x + MOUSE_BOUNDS.width + 5.0,
            MOUSE_BOUNDS.y + MOUSE_BOUNDS.height + 5.0,
        ));
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);
        let moved = mouse::Event::CursorMoved { position: Point::ORIGIN };

        for event in [&LEFT_PRESS, &release, &moved] {
            assert!(
                editor
                    .handle_mouse_event(event, MOUSE_BOUNDS, &outside)
                    .is_none(),
                "{event:?} outside the canvas must not be handled"
            );
            assert!(
                editor
                    .handle_mouse_event(
                        event,
                        MOUSE_BOUNDS,
                        &mouse::Cursor::Unavailable
                    )
                    .is_none(),
                "{event:?} with no cursor must not be handled"
            );
        }
    }

    #[test]
    fn test_right_click_requests_the_context_menu() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let right_press = mouse::Event::ButtonPressed(mouse::Button::Right);
        let cursor = cursor_at(15.0, 25.0);

        let message = mouse_message(&editor, &right_press, &cursor);
        assert!(
            matches!(message, Some(Message::ContextMenuRequested(_))),
            "expected a ContextMenuRequested, got {message:?}"
        );
        assert_position(&message, 15.0, 25.0);
        assert!(matches!(
            mouse_status(&editor, &right_press, &cursor),
            Some(event::Status::Captured)
        ));
    }

    #[test]
    fn test_cursor_moved_hovers_when_idle_and_drags_while_dragging() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        let moved = mouse::Event::CursorMoved {
            // Ignored by the handler, which reads the cursor rather than the
            // event payload; set to a decoy value to prove it.
            position: Point::new(999.0, 999.0),
        };
        let cursor = cursor_at(50.0, 60.0);

        assert!(!editor.is_dragging);
        let message = mouse_message(&editor, &moved, &cursor);
        assert!(
            matches!(message, Some(Message::MouseHover(_))),
            "expected a MouseHover, got {message:?}"
        );
        assert_position(&message, 50.0, 60.0);
        // Hover must stay uncaptured or it would swallow motion events the
        // host may need.
        assert!(matches!(
            mouse_status(&editor, &moved, &cursor),
            Some(event::Status::Ignored)
        ));

        editor.is_dragging = true;
        let message = mouse_message(&editor, &moved, &cursor);
        assert!(
            matches!(message, Some(Message::MouseDrag(_))),
            "expected a MouseDrag, got {message:?}"
        );
        assert_position(&message, 50.0, 60.0);
        assert!(matches!(
            mouse_status(&editor, &moved, &cursor),
            Some(event::Status::Captured)
        ));
    }

    #[test]
    fn test_left_release_is_handled_only_over_the_canvas() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let release = mouse::Event::ButtonReleased(mouse::Button::Left);

        assert!(matches!(
            mouse_message(&editor, &release, &cursor_at(5.0, 5.0)),
            Some(Message::MouseRelease)
        ));
        assert!(matches!(
            mouse_status(&editor, &release, &cursor_at(5.0, 5.0)),
            Some(event::Status::Captured)
        ));
    }

    #[test]
    fn test_alt_click_adds_a_cursor_unless_vim_is_enabled() {
        let mut editor = CodeEditor::new("one\ntwo", "txt");
        editor.modifiers.set(keyboard::Modifiers::ALT);
        let cursor = cursor_at(70.0, 10.0);

        let message = mouse_message(&editor, &LEFT_PRESS, &cursor);
        assert!(
            matches!(message, Some(Message::AltClick(_))),
            "expected an AltClick, got {message:?}"
        );
        assert_position(&message, 70.0, 10.0);

        // Vim owns its own multi-cursor model, so Alt+Click degrades to a
        // plain click rather than adding an editor cursor behind its back.
        editor.set_vim_enabled(true);
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_control_click_jumps_to_definition_without_capturing() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        // COMMAND is CTRL on every platform but macOS, where the handler reads
        // `command()` instead; setting both exercises the same branch on each.
        editor
            .modifiers
            .set(keyboard::Modifiers::CTRL | keyboard::Modifiers::COMMAND);
        let cursor = cursor_at(80.0, 12.0);

        let message = mouse_message(&editor, &LEFT_PRESS, &cursor);
        assert!(
            matches!(message, Some(Message::JumpClick(_))),
            "expected a JumpClick, got {message:?}"
        );
        assert_position(&message, 80.0, 12.0);

        // Left uncaptured: the host resolves the jump and may decline it.
        assert!(matches!(
            mouse_status(&editor, &LEFT_PRESS, &cursor),
            Some(event::Status::Ignored)
        ));
    }

    #[test]
    fn test_repeated_clicks_at_one_spot_escalate_to_double_then_triple() {
        let editor = CodeEditor::new("one two three", "txt");
        let cursor = cursor_at(90.0, 5.0);

        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::DoubleClick(_))
        ));
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::TripleClick(_))
        ));
        // A fourth click wraps back to a single click rather than sticking at
        // triple.
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_a_click_far_from_the_previous_one_stays_a_single_click() {
        let editor = CodeEditor::new("one two three", "txt");

        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(20.0, 5.0)),
            Some(Message::MouseClick(_))
        ));
        // Beyond the 6px tolerance, so this is a new click sequence.
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor_at(120.0, 80.0)),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_clicking_the_fold_chevron_toggles_the_block() {
        let editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");
        assert!(editor.folding_enabled);

        // The chevron column sits between the line-number area and the text;
        // aim at its middle, on the first visual line.
        let x = f32::midpoint(
            editor.line_number_gutter_width(),
            editor.gutter_width(),
        );
        let y = editor.line_height * 0.5;
        let cursor = cursor_at(x, y);

        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::ToggleFold(0))
        ));

        // The chevron wins over the modifier-click bindings: Alt+Click on it
        // toggles the fold instead of dropping a cursor in the gutter.
        editor.modifiers.set(keyboard::Modifiers::ALT);
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::ToggleFold(0))
        ));
    }

    #[test]
    fn test_clicking_the_text_area_is_not_a_fold_toggle() {
        let editor = CodeEditor::new("fn a() {\n    body\n}\n", "rs");

        // Same line, but past the gutter: this is ordinary caret placement.
        let cursor =
            cursor_at(editor.gutter_width() + 20.0, editor.line_height * 0.5);
        assert!(matches!(
            mouse_message(&editor, &LEFT_PRESS, &cursor),
            Some(Message::MouseClick(_))
        ));
    }

    #[test]
    fn test_unhandled_mouse_events_are_ignored() {
        let editor = CodeEditor::new("one\ntwo", "txt");
        let cursor = cursor_at(30.0, 30.0);
        let events = [
            mouse::Event::ButtonPressed(mouse::Button::Middle),
            mouse::Event::ButtonReleased(mouse::Button::Right),
            mouse::Event::CursorEntered,
            mouse::Event::CursorLeft,
            mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 },
            },
        ];

        for event in &events {
            assert!(
                editor
                    .handle_mouse_event(event, MOUSE_BOUNDS, &cursor)
                    .is_none(),
                "{event:?} should not be handled here"
            );
        }
    }
}
