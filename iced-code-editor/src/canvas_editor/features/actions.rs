//! State and shortcut hints shared by the editor's action surfaces.
//!
//! Two surfaces expose the same built-in editing actions: the right-click
//! context menu ([`super::context_menu`]) and the command palette
//! ([`super::command_palette`]). Both need to know which actions are usable
//! right now, and both display the same keyboard-shortcut hints, so the
//! availability snapshot and the shortcut strings live here instead of being
//! spelled out twice.

use crate::canvas_editor::CodeEditor;

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
