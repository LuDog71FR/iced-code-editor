//! State and shortcut hints shared by the editor's action surfaces.
//!
//! Two surfaces expose the same built-in editing actions: the right-click
//! context menu ([`super::context_menu`]) and the command palette
//! ([`super::command_palette`]). Both need to know which actions are usable
//! right now, and both display the same keyboard-shortcut hints, so the
//! availability snapshot and the shortcut strings live here instead of being
//! spelled out twice.

use crate::canvas_editor::{CodeEditor, Message};
use crate::i18n::Translations;

/// Availability of the built-in editor actions at the moment an action
/// surface is opened.
///
/// Captured once when the menu or palette is built rather than read live:
/// both surfaces render from an owned snapshot, so they cannot borrow the
/// editor while it is being updated.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ActionContext {
    /// Whether the history has an operation to undo.
    pub(crate) can_undo: bool,
    /// Whether the history has an undone operation to redo.
    pub(crate) can_redo: bool,
    /// Whether at least one cursor currently holds a selection.
    pub(crate) has_selection: bool,
    /// Whether the buffer holds anything at all.
    pub(crate) has_content: bool,
    /// Whether the host opted into the reveal-in-file-manager action.
    pub(crate) reveal_in_file_manager_enabled: bool,
    /// Whether the search/replace dialogs are available.
    pub(crate) search_replace_enabled: bool,
    /// Whether code folding is available.
    pub(crate) folding_enabled: bool,
}

impl CodeEditor {
    /// Snapshots which built-in actions are currently available.
    pub(crate) fn action_context(&self) -> ActionContext {
        ActionContext {
            can_undo: self.history.can_undo(),
            can_redo: self.history.can_redo(),
            has_selection: self
                .cursors
                .iter()
                .any(|cursor| cursor.has_selection()),
            has_content: self.buffer.line_count() > 1
                || self.buffer.line_len(0) > 0,
            reveal_in_file_manager_enabled: self
                .reveal_in_file_manager_enabled(),
            search_replace_enabled: self.search_replace_enabled,
            folding_enabled: self.folding_enabled,
        }
    }
}

// =============================================================================
// Keyboard shortcut hints
// =============================================================================
//
// Display-only strings: binding the keys is `input::events`' job. They are
// kept next to each other so a rebinding there has one obvious place to
// update, and so the context menu and the palette can never drift apart on
// how the same action is spelled.

#[cfg(target_os = "macos")]
pub(crate) const UNDO_SHORTCUT: &str = "⌘Z";
#[cfg(not(target_os = "macos"))]
pub(crate) const UNDO_SHORTCUT: &str = "Ctrl+Z";

#[cfg(target_os = "macos")]
pub(crate) const REDO_SHORTCUT: &str = "⇧⌘Z";
#[cfg(not(target_os = "macos"))]
pub(crate) const REDO_SHORTCUT: &str = "Ctrl+Y";

#[cfg(target_os = "macos")]
pub(crate) const CUT_SHORTCUT: &str = "⌘X";
#[cfg(not(target_os = "macos"))]
pub(crate) const CUT_SHORTCUT: &str = "Ctrl+X";

#[cfg(target_os = "macos")]
pub(crate) const COPY_SHORTCUT: &str = "⌘C";
#[cfg(not(target_os = "macos"))]
pub(crate) const COPY_SHORTCUT: &str = "Ctrl+C";

#[cfg(target_os = "macos")]
pub(crate) const PASTE_SHORTCUT: &str = "⌘V";
#[cfg(not(target_os = "macos"))]
pub(crate) const PASTE_SHORTCUT: &str = "Ctrl+V";

#[cfg(target_os = "macos")]
pub(crate) const SELECT_ALL_SHORTCUT: &str = "⌘A";
#[cfg(not(target_os = "macos"))]
pub(crate) const SELECT_ALL_SHORTCUT: &str = "Ctrl+A";

#[cfg(target_os = "macos")]
pub(crate) const SAVE_SHORTCUT: &str = "⌘S";
#[cfg(not(target_os = "macos"))]
pub(crate) const SAVE_SHORTCUT: &str = "Ctrl+S";

#[cfg(target_os = "macos")]
pub(crate) const FIND_SHORTCUT: &str = "⌘F";
#[cfg(not(target_os = "macos"))]
pub(crate) const FIND_SHORTCUT: &str = "Ctrl+F";

#[cfg(target_os = "macos")]
pub(crate) const REPLACE_SHORTCUT: &str = "⌘H";
#[cfg(not(target_os = "macos"))]
pub(crate) const REPLACE_SHORTCUT: &str = "Ctrl+H";

#[cfg(target_os = "macos")]
pub(crate) const GOTO_LINE_SHORTCUT: &str = "⌘G";
#[cfg(not(target_os = "macos"))]
pub(crate) const GOTO_LINE_SHORTCUT: &str = "Ctrl+G";

#[cfg(target_os = "macos")]
pub(crate) const TOGGLE_COMMENT_SHORTCUT: &str = "⌘/";
#[cfg(not(target_os = "macos"))]
pub(crate) const TOGGLE_COMMENT_SHORTCUT: &str = "Ctrl+/";

#[cfg(target_os = "macos")]
pub(crate) const MOVE_LINE_UP_SHORTCUT: &str = "⌥↑";
#[cfg(not(target_os = "macos"))]
pub(crate) const MOVE_LINE_UP_SHORTCUT: &str = "Alt+↑";

#[cfg(target_os = "macos")]
pub(crate) const MOVE_LINE_DOWN_SHORTCUT: &str = "⌥↓";
#[cfg(not(target_os = "macos"))]
pub(crate) const MOVE_LINE_DOWN_SHORTCUT: &str = "Alt+↓";

#[cfg(target_os = "macos")]
pub(crate) const DUPLICATE_LINE_UP_SHORTCUT: &str = "⇧⌥↑";
#[cfg(not(target_os = "macos"))]
pub(crate) const DUPLICATE_LINE_UP_SHORTCUT: &str = "Shift+Alt+↑";

#[cfg(target_os = "macos")]
pub(crate) const DUPLICATE_LINE_DOWN_SHORTCUT: &str = "⇧⌥↓";
#[cfg(not(target_os = "macos"))]
pub(crate) const DUPLICATE_LINE_DOWN_SHORTCUT: &str = "Shift+Alt+↓";

// Folding and multi-cursor bindings require the physical Control key, not the
// platform command key, so their macOS hints use ⌃ rather than ⌘.
#[cfg(target_os = "macos")]
pub(crate) const FOLD_AT_CURSOR_SHORTCUT: &str = "⌃.";
#[cfg(not(target_os = "macos"))]
pub(crate) const FOLD_AT_CURSOR_SHORTCUT: &str = "Ctrl+.";

#[cfg(target_os = "macos")]
pub(crate) const FOLD_ALL_SHORTCUT: &str = "⌃K";
#[cfg(not(target_os = "macos"))]
pub(crate) const FOLD_ALL_SHORTCUT: &str = "Ctrl+K";

#[cfg(target_os = "macos")]
pub(crate) const UNFOLD_ALL_SHORTCUT: &str = "⌃J";
#[cfg(not(target_os = "macos"))]
pub(crate) const UNFOLD_ALL_SHORTCUT: &str = "Ctrl+J";

#[cfg(target_os = "macos")]
pub(crate) const ADD_CURSOR_ABOVE_SHORTCUT: &str = "⌃⌥↑";
#[cfg(not(target_os = "macos"))]
pub(crate) const ADD_CURSOR_ABOVE_SHORTCUT: &str = "Ctrl+Alt+↑";

#[cfg(target_os = "macos")]
pub(crate) const ADD_CURSOR_BELOW_SHORTCUT: &str = "⌃⌥↓";
#[cfg(not(target_os = "macos"))]
pub(crate) const ADD_CURSOR_BELOW_SHORTCUT: &str = "Ctrl+Alt+↓";

#[cfg(target_os = "macos")]
pub(crate) const SELECT_NEXT_OCCURRENCE_SHORTCUT: &str = "⌘D";
#[cfg(not(target_os = "macos"))]
pub(crate) const SELECT_NEXT_OCCURRENCE_SHORTCUT: &str = "Ctrl+D";

#[cfg(target_os = "macos")]
pub(crate) const TOGGLE_VIM_MODE_SHORTCUT: &str = "⌥⌘V";
#[cfg(not(target_os = "macos"))]
pub(crate) const TOGGLE_VIM_MODE_SHORTCUT: &str = "Ctrl+Alt+V";

// =============================================================================
// Shared actions
// =============================================================================

/// A built-in editing action that both surfaces offer.
///
/// The context menu and the command palette each present these seven in their
/// own order and with their own idea of what to do when one is unavailable —
/// the menu dims the row, the palette leaves it out. What they must *not*
/// disagree on is the binding itself: which label goes with which shortcut
/// hint, which [`Message`] it sends, and what makes it available. That binding
/// lives here, once, so the two surfaces cannot drift apart on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SharedAction {
    /// Undo the last operation.
    Undo,
    /// Redo the last undone operation.
    Redo,
    /// Cut the selection to the clipboard.
    Cut,
    /// Copy the selection to the clipboard.
    Copy,
    /// Paste the clipboard at the cursor.
    Paste,
    /// Select the whole buffer.
    SelectAll,
    /// Show the current file in the platform's file manager.
    RevealInFileManager,
}

impl SharedAction {
    /// Every shared action, in no particular display order.
    ///
    /// Test-only: each surface spells out its own order, so nothing in the
    /// rendering path iterates the whole set. It exists so the cross-surface
    /// test below is exhaustive by construction — a new variant that is not
    /// added here fails to compile against the array's length.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 7] = [
        Self::Undo,
        Self::Redo,
        Self::Cut,
        Self::Copy,
        Self::Paste,
        Self::SelectAll,
        Self::RevealInFileManager,
    ];

    /// Returns the translated label shown for this action.
    ///
    /// # Arguments
    ///
    /// * `translations` - The active translation catalogue
    pub(crate) fn label(self, translations: &Translations) -> String {
        match self {
            Self::Undo => translations.context_menu_undo(),
            Self::Redo => translations.context_menu_redo(),
            Self::Cut => translations.context_menu_cut(),
            Self::Copy => translations.context_menu_copy(),
            Self::Paste => translations.context_menu_paste(),
            Self::SelectAll => translations.context_menu_select_all(),
            Self::RevealInFileManager => {
                translations.context_menu_reveal_in_file_manager()
            }
        }
    }

    /// Returns the keyboard-shortcut hint, empty when the action has none.
    pub(crate) fn shortcut(self) -> &'static str {
        match self {
            Self::Undo => UNDO_SHORTCUT,
            Self::Redo => REDO_SHORTCUT,
            Self::Cut => CUT_SHORTCUT,
            Self::Copy => COPY_SHORTCUT,
            Self::Paste => PASTE_SHORTCUT,
            Self::SelectAll => SELECT_ALL_SHORTCUT,
            // Reached from the menu and the palette only; no key is bound.
            Self::RevealInFileManager => "",
        }
    }

    /// Returns the message this action sends when it is run.
    pub(crate) fn message(self) -> Message {
        match self {
            Self::Undo => Message::Undo,
            Self::Redo => Message::Redo,
            Self::Cut => Message::Cut,
            Self::Copy => Message::Copy,
            // An empty payload is not an empty paste: it asks
            // `CodeEditor::handle_paste_msg` to read the clipboard and send a
            // second `Paste` carrying what it found.
            Self::Paste => Message::Paste(String::new()),
            Self::SelectAll => Message::SelectAll,
            Self::RevealInFileManager => Message::RevealInFileManager,
        }
    }

    /// Returns whether the action can be run in `context`.
    ///
    /// Paste is always available: whether the clipboard holds anything is only
    /// known once it has been read.
    ///
    /// # Arguments
    ///
    /// * `context` - The availability snapshot taken when the surface opened
    pub(crate) fn is_available(self, context: ActionContext) -> bool {
        match self {
            Self::Undo => context.can_undo,
            Self::Redo => context.can_redo,
            Self::Cut | Self::Copy => context.has_selection,
            Self::Paste => true,
            Self::SelectAll => context.has_content,
            Self::RevealInFileManager => context.reveal_in_file_manager_enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::Message;

    #[test]
    fn test_action_context_reports_history_and_selection_state() {
        let mut editor = CodeEditor::new("hello", "rs");
        let empty = editor.action_context();
        assert!(!empty.can_undo);
        assert!(!empty.can_redo);
        assert!(!empty.has_selection);
        assert!(empty.has_content);

        // Paste rather than type: typed characters stay in an open undo
        // group until the run ends, so nothing is undoable yet.
        let _ = editor.update(&Message::Paste("!".to_string()));
        let _ = editor.update(&Message::SelectAll);

        let edited = editor.action_context();
        assert!(edited.can_undo);
        assert!(edited.has_selection);
    }

    #[test]
    fn test_action_context_reports_empty_buffer_as_contentless() {
        let editor = CodeEditor::new("", "rs");
        assert!(!editor.action_context().has_content);
    }

    #[test]
    fn test_action_context_mirrors_feature_toggles() {
        let mut editor =
            CodeEditor::new("hello", "rs").with_folding_enabled(false);
        editor.set_search_replace_enabled(false);

        let context = editor.action_context();
        assert!(!context.search_replace_enabled);
        assert!(!context.folding_enabled);
        assert!(!context.reveal_in_file_manager_enabled);
    }
}
