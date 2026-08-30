//! Keyboard shortcut recognition for [`CodeEditor`].
//!
//! This module answers "which editor command did this key combination ask
//! for?"; delivering the answer -- focus checks, IME suppression, character
//! input and the mouse and IME routes -- is [`super::events`]'s job. The
//! split keeps the shortcut table and the tests pinning its precedence rules
//! together, away from the event plumbing they do not depend on.

use iced::keyboard;
use iced::widget::canvas::Action;

use crate::canvas_editor::features::vim::VimMode;
use crate::canvas_editor::{CodeEditor, Message};

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

/// Returns `true` when the platform's "command" modifier is held.
///
/// `Modifiers::command()` alone is not enough: it maps to Cmd on macOS and to
/// Ctrl elsewhere, but a keyboard reporting a raw Ctrl press on macOS still
/// has to work for the shortcuts that are documented as `Ctrl/Cmd+X`. Every
/// such shortcut in this file goes through here so the two-way check is
/// spelled out once instead of at each group.
///
/// # Examples
///
/// ```text
/// // Ctrl+C on Linux and Cmd+C on macOS both answer `true`.
/// assert!(command_pressed(&keyboard::Modifiers::CTRL));
/// ```
pub(super) fn command_pressed(modifiers: &keyboard::Modifiers) -> bool {
    modifiers.command() || modifiers.control()
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
    if command_pressed(modifiers)
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
    if command_pressed(modifiers)
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
    if (command_pressed(modifiers) && is_key_char(key, modified_key, "c"))
        || (modifiers.control()
            && matches!(
                key,
                keyboard::Key::Named(keyboard::key::Named::Insert)
            ))
    {
        return Some(Action::publish(Message::Copy).and_capture());
    }

    if command_pressed(modifiers) && is_key_char(key, modified_key, "x") {
        return Some(Action::publish(Message::Cut).and_capture());
    }

    if command_pressed(modifiers) && is_key_char(key, modified_key, "a") {
        return Some(Action::publish(Message::SelectAll).and_capture());
    }

    if (command_pressed(modifiers) && is_key_char(key, modified_key, "v"))
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
    if command_pressed(modifiers) && is_key_char(key, modified_key, "d") {
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
    // On French AZERTY `/` is Shift+`:` and only appears in `modified_key`;
    // see `is_key_char`.
    if command_pressed(modifiers) && is_key_char(key, modified_key, "/") {
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
    if command_pressed(modifiers)
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::Home))
    {
        return Some(Action::publish(Message::CtrlHome).and_capture());
    }

    if command_pressed(modifiers)
        && matches!(key, keyboard::Key::Named(keyboard::key::Named::End))
    {
        return Some(Action::publish(Message::CtrlEnd).and_capture());
    }

    None
}

impl CodeEditor {
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
    pub(super) fn handle_keyboard_shortcuts(
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
        if command_pressed(modifiers)
            && !modifiers.shift()
            && is_key_char(key, modified_key, "z")
        {
            return Some(Action::publish(Message::Undo).and_capture());
        }

        if command_pressed(modifiers)
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

    /// Handles the search/replace, goto-line and command-palette dialogs:
    /// opening them (`Ctrl/Cmd+F`, `Ctrl/Cmd+H`, `Ctrl/Cmd+G`,
    /// `Ctrl/Cmd+Shift+P`), cycling the search dialog's fields while it is
    /// open (`Tab`/`Shift+Tab`), and find next/previous (`F3`/`Shift+F3`).
    fn dialog_shortcut(
        &self,
        key: &keyboard::Key,
        modified_key: &keyboard::Key,
        modifiers: &keyboard::Modifiers,
    ) -> Option<Action<Message>> {
        if command_pressed(modifiers)
            && modifiers.shift()
            && is_key_char(key, modified_key, "p")
            && self.command_palette_enabled
        {
            return Some(
                Action::publish(Message::OpenCommandPalette).and_capture(),
            );
        }

        if command_pressed(modifiers)
            && is_key_char(key, modified_key, "f")
            && self.search_replace_enabled
        {
            return Some(Action::publish(Message::OpenSearch).and_capture());
        }

        if command_pressed(modifiers)
            && is_key_char(key, modified_key, "h")
            && self.search_replace_enabled
        {
            return Some(
                Action::publish(Message::OpenSearchReplace).and_capture(),
            );
        }

        if command_pressed(modifiers) && is_key_char(key, modified_key, "g") {
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

    /// Handles Escape: closes the innermost open dialog (command palette,
    /// then go-to-line, then search), collapses a multi-cursor
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

        let message = if self.command_palette_state.is_open {
            Some(Message::CloseCommandPalette)
        } else if self.goto_line_state.is_open {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::event;

    #[test]
    fn test_command_pressed_accepts_either_command_or_control() {
        assert!(command_pressed(&keyboard::Modifiers::CTRL));
        assert!(command_pressed(&keyboard::Modifiers::COMMAND));
        assert!(command_pressed(
            &(keyboard::Modifiers::CTRL | keyboard::Modifiers::SHIFT)
        ));
        assert!(!command_pressed(&keyboard::Modifiers::SHIFT));
        assert!(!command_pressed(&keyboard::Modifiers::ALT));
        assert!(!command_pressed(&keyboard::Modifiers::default()));
    }

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
}
