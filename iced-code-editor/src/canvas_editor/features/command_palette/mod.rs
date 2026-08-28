//! State and command registry for the command palette.
//!
//! The palette is a keyboard-driven launcher for editor actions: `Ctrl+Shift+P`
//! opens a filtered list of every command available right now, typing narrows
//! it, and `Enter` runs the highlighted one. It exists so an action is
//! reachable without knowing its shortcut — the shortcut is shown beside each
//! row, so the palette also teaches them.
//!
//! The registry is extensible: a host application registers its own commands
//! with [`CodeEditor::set_custom_command_palette_entries`] and receives
//! [`Message::CommandPaletteAction`] carrying the entry's `id` when one is run.

pub(crate) mod dialog;
mod update;

use iced::widget::Id;

use super::actions::{
    ADD_CURSOR_ABOVE_SHORTCUT, ADD_CURSOR_BELOW_SHORTCUT, ActionContext,
    DUPLICATE_LINE_DOWN_SHORTCUT, DUPLICATE_LINE_UP_SHORTCUT, FIND_SHORTCUT,
    FOLD_ALL_SHORTCUT, FOLD_AT_CURSOR_SHORTCUT, GOTO_LINE_SHORTCUT,
    MOVE_LINE_DOWN_SHORTCUT, MOVE_LINE_UP_SHORTCUT, REPLACE_SHORTCUT,
    SAVE_SHORTCUT, SELECT_NEXT_OCCURRENCE_SHORTCUT, SharedAction,
    TOGGLE_COMMENT_SHORTCUT, TOGGLE_VIM_MODE_SHORTCUT, UNFOLD_ALL_SHORTCUT,
};
use super::context_menu::ContextMenuItem;
use crate::canvas_editor::{CodeEditor, Message};
use crate::i18n::Translations;

/// What running a palette row does.
#[derive(Debug, Clone)]
pub(crate) enum PaletteAction {
    /// A built-in editor action, applied by re-entering [`CodeEditor::update`].
    Builtin(Box<Message>),
    /// A host-registered action, forwarded as [`Message::CommandPaletteAction`]
    /// carrying the entry's stable identifier.
    Custom(String),
}

/// One row of the palette: what is displayed, and what running it does.
#[derive(Debug, Clone)]
pub(crate) struct PaletteEntry {
    /// Text shown for the command.
    pub(crate) label: String,
    /// Keyboard shortcut hint shown to the right, empty when unbound.
    pub(crate) shortcut: String,
    /// The action performed when the row is run.
    pub(crate) action: PaletteAction,
}

impl PaletteEntry {
    /// Builds a row running a built-in editor message.
    fn builtin(
        label: String,
        shortcut: &'static str,
        message: Message,
    ) -> Self {
        Self::builtin_owned(label, shortcut.to_string(), message)
    }

    /// Builds a row whose shortcut hint is not a `'static` string.
    ///
    /// The shared actions carry theirs through [`SharedAction::shortcut`],
    /// which is `&'static str` but reaches here as an owned `String` after
    /// going through the same mapping the context menu uses.
    fn builtin_owned(
        label: String,
        shortcut: String,
        message: Message,
    ) -> Self {
        Self {
            label,
            shortcut,
            action: PaletteAction::Builtin(Box::new(message)),
        }
    }
}

/// State owned by the command palette.
#[derive(Debug, Clone)]
pub(crate) struct CommandPaletteState {
    /// Current filter text typed by the user.
    pub(crate) query: String,
    /// Whether the palette is visible.
    pub(crate) is_open: bool,
    /// Index of the highlighted row within the filtered list.
    pub(crate) selected: usize,
    /// Stable input ID used for focus and selection operations.
    pub(crate) input_id: Id,
    /// Stable ID of the result list, used to scroll the highlighted row
    /// into view while navigating with the keyboard.
    pub(crate) scrollable_id: Id,
    /// Index of the row currently scrolled to the top of the list.
    ///
    /// Tracked rather than derived from [`Self::selected`] so the list only
    /// moves when the highlight would otherwise leave the window — see
    /// [`window_origin`].
    pub(crate) first_visible_row: usize,
}

impl Default for CommandPaletteState {
    fn default() -> Self {
        Self {
            query: String::new(),
            is_open: false,
            selected: 0,
            input_id: Id::unique(),
            scrollable_id: Id::unique(),
            first_visible_row: 0,
        }
    }
}

impl CommandPaletteState {
    /// Creates a closed palette state.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Opens the palette with an empty query and the first row highlighted.
    ///
    /// The query is always reset: a palette that reopened on the previous
    /// search would hide most commands behind a filter the user did not
    /// type this time.
    pub(crate) fn open(&mut self) {
        self.query.clear();
        self.select_first_row();
        self.is_open = true;
    }

    /// Highlights the first row and scrolls the list back to the top.
    ///
    /// The two always move together: a highlight on row 0 under a list still
    /// scrolled to row 12 shows a window with nothing highlighted in it.
    pub(crate) fn select_first_row(&mut self) {
        self.selected = 0;
        self.first_visible_row = 0;
    }

    /// Closes the palette, leaving its query for the next [`Self::open`] to
    /// clear.
    pub(crate) fn close(&mut self) {
        self.is_open = false;
    }

    /// Moves the highlight by `delta` rows over a list of `len` entries,
    /// wrapping around at both ends, and scrolls the list only as far as
    /// keeping the new highlight visible requires.
    ///
    /// Wrapping matches the LSP completion menu, and makes the last command
    /// reachable with a single `Up` from the top.
    pub(crate) fn navigate(&mut self, delta: i32, len: usize) {
        if len == 0 {
            self.select_first_row();
            return;
        }
        let wrap = i32::try_from(len).unwrap_or(i32::MAX);
        let current = i32::try_from(self.selected).unwrap_or(0);
        let next = (current + delta).rem_euclid(wrap);
        self.selected = usize::try_from(next).unwrap_or(0);
        self.first_visible_row =
            window_origin(self.selected, self.first_visible_row, len);
    }
}

/// Returns the row the list should be scrolled to so that `selected` is
/// visible, moving as little as possible.
///
/// The window shows [`dialog::MAX_VISIBLE_ROWS`] rows starting at `origin`.
/// A highlight already inside that window leaves it where it is — scrolling on
/// every keystroke would make the list slide under the reader for no reason.
/// A highlight above the window becomes the new top row; one below it becomes
/// the new bottom row, which is what wrapping from the first row to the last
/// produces.
///
/// # Arguments
///
/// * `selected` - Index of the highlighted row
/// * `origin` - Row currently scrolled to the top of the list
/// * `len` - Number of rows in the list
///
/// # Returns
///
/// The row to scroll to the top, never far enough to leave a gap below the
/// last row
fn window_origin(selected: usize, origin: usize, len: usize) -> usize {
    // Scrolling past this would show empty space under the final row.
    let last_origin = len.saturating_sub(dialog::MAX_VISIBLE_ROWS);
    let origin = origin.min(last_origin);

    if selected < origin {
        selected
    } else if selected >= origin + dialog::MAX_VISIBLE_ROWS {
        (selected + 1 - dialog::MAX_VISIBLE_ROWS).min(last_origin)
    } else {
        origin
    }
}

/// Returns whether `label` matches `query`.
///
/// The match is a case-insensitive subsequence test, not a substring test:
/// the query's characters must appear in `label` in order but need not be
/// adjacent, so `tc` finds "Toggle Line Comment" and `fldall` finds "Fold
/// All". An empty query matches everything.
fn matches_query(label: &str, query: &str) -> bool {
    let mut remaining = query.chars().flat_map(char::to_lowercase).peekable();
    for candidate in label.chars().flat_map(char::to_lowercase) {
        match remaining.peek() {
            Some(wanted) if *wanted == candidate => {
                remaining.next();
            }
            Some(_) => {}
            None => return true,
        }
    }
    remaining.peek().is_none()
}

/// Builds the palette rows for the host-registered commands.
///
/// Disabled entries are dropped rather than dimmed: unlike a context menu,
/// whose fixed shape helps muscle memory, the palette is a search result
/// list, so every row it shows should be runnable.
fn custom_entries(entries: &[ContextMenuItem]) -> Vec<PaletteEntry> {
    entries
        .iter()
        .filter(|item| item.enabled)
        .map(|item| PaletteEntry {
            label: item.label.clone(),
            shortcut: item.shortcut.clone().unwrap_or_default(),
            action: PaletteAction::Custom(item.id.clone()),
        })
        .collect()
}

/// Turns a shared action into a palette row.
///
/// The binding it renders — label, shortcut hint, message — comes from
/// [`SharedAction`], the same source the context menu reads.
fn shared_row(
    action: SharedAction,
    translations: &Translations,
) -> PaletteEntry {
    PaletteEntry::builtin_owned(
        action.label(translations),
        action.shortcut().to_string(),
        action.message(),
    )
}

/// Builds the palette rows for the built-in editor commands that are
/// available in `context`.
fn default_entries(
    context: ActionContext,
    translations: &Translations,
) -> Vec<PaletteEntry> {
    let mut entries = vec![
        PaletteEntry::builtin(
            translations.command_palette_save(),
            SAVE_SHORTCUT,
            Message::WriteRequested,
        ),
        PaletteEntry::builtin(
            translations.command_palette_toggle_comment(),
            TOGGLE_COMMENT_SHORTCUT,
            Message::ToggleComment,
        ),
        PaletteEntry::builtin(
            translations.command_palette_move_line_up(),
            MOVE_LINE_UP_SHORTCUT,
            Message::MoveLineUp,
        ),
        PaletteEntry::builtin(
            translations.command_palette_move_line_down(),
            MOVE_LINE_DOWN_SHORTCUT,
            Message::MoveLineDown,
        ),
        PaletteEntry::builtin(
            translations.command_palette_duplicate_line_up(),
            DUPLICATE_LINE_UP_SHORTCUT,
            Message::DuplicateLineUp,
        ),
        PaletteEntry::builtin(
            translations.command_palette_duplicate_line_down(),
            DUPLICATE_LINE_DOWN_SHORTCUT,
            Message::DuplicateLineDown,
        ),
        PaletteEntry::builtin(
            translations.command_palette_goto_line(),
            GOTO_LINE_SHORTCUT,
            Message::OpenGotoLine,
        ),
        PaletteEntry::builtin(
            translations.command_palette_add_cursor_above(),
            ADD_CURSOR_ABOVE_SHORTCUT,
            Message::AddCursorAbove,
        ),
        PaletteEntry::builtin(
            translations.command_palette_add_cursor_below(),
            ADD_CURSOR_BELOW_SHORTCUT,
            Message::AddCursorBelow,
        ),
        PaletteEntry::builtin(
            translations.command_palette_select_next_occurrence(),
            SELECT_NEXT_OCCURRENCE_SHORTCUT,
            Message::SelectNextOccurrence,
        ),
        PaletteEntry::builtin(
            translations.command_palette_toggle_vim_mode(),
            TOGGLE_VIM_MODE_SHORTCUT,
            Message::ToggleVimMode,
        ),
    ];

    // Paste keeps its place among the always-available commands: it is the
    // only shared action with no availability condition.
    entries.push(shared_row(SharedAction::Paste, translations));

    // The rest of what the context menu also offers. Labels, shortcuts and
    // messages come from `SharedAction`, so the two surfaces cannot disagree
    // on a binding; what belongs to the palette is *dropping* an unavailable
    // action instead of dimming it, since a search result list should only
    // offer rows that can be run.
    entries.extend(
        [
            SharedAction::Undo,
            SharedAction::Redo,
            SharedAction::Cut,
            SharedAction::Copy,
            SharedAction::SelectAll,
        ]
        .into_iter()
        .filter(|action| action.is_available(context))
        .map(|action| shared_row(action, translations)),
    );

    if context.search_replace_enabled {
        entries.push(PaletteEntry::builtin(
            translations.command_palette_find(),
            FIND_SHORTCUT,
            Message::OpenSearch,
        ));
        entries.push(PaletteEntry::builtin(
            translations.command_palette_replace(),
            REPLACE_SHORTCUT,
            Message::OpenSearchReplace,
        ));
    }
    if context.folding_enabled {
        entries.push(PaletteEntry::builtin(
            translations.command_palette_fold_at_cursor(),
            FOLD_AT_CURSOR_SHORTCUT,
            Message::ToggleFoldAtCursor,
        ));
        entries.push(PaletteEntry::builtin(
            translations.command_palette_fold_all(),
            FOLD_ALL_SHORTCUT,
            Message::FoldAll,
        ));
        entries.push(PaletteEntry::builtin(
            translations.command_palette_unfold_all(),
            UNFOLD_ALL_SHORTCUT,
            Message::UnfoldAll,
        ));
    }

    // Reveal comes last: it acts on the file rather than on the text.
    if SharedAction::RevealInFileManager.is_available(context) {
        entries
            .push(shared_row(SharedAction::RevealInFileManager, translations));
    }

    entries
}

/// Assembles the full command list: host-registered entries first, then the
/// built-in ones when they are enabled.
fn build_entries(
    custom: &[ContextMenuItem],
    default_command_palette_enabled: bool,
    context: ActionContext,
    translations: &Translations,
) -> Vec<PaletteEntry> {
    let mut entries = custom_entries(custom);
    if default_command_palette_enabled {
        entries.extend(default_entries(context, translations));
    }
    entries
}

impl CodeEditor {
    /// Returns the palette rows matching the current query, in display order.
    pub(crate) fn command_palette_entries(&self) -> Vec<PaletteEntry> {
        build_entries(
            self.custom_command_palette_entries(),
            self.default_command_palette_enabled(),
            self.action_context(),
            &self.translations,
        )
        .into_iter()
        .filter(|entry| {
            matches_query(&entry.label, self.command_palette_state.query.trim())
        })
        .collect()
    }

    /// Opens the command palette programmatically.
    ///
    /// Use this to wire a menu item or toolbar button, alongside the built-in
    /// `Ctrl/Cmd+Shift+P` shortcut.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that focuses the palette's input
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// let _task = editor.open_command_palette();
    /// ```
    pub fn open_command_palette(&mut self) -> iced::Task<Message> {
        self.update(&Message::OpenCommandPalette)
    }

    /// Closes the command palette without running anything.
    ///
    /// Safe to call when the palette is already closed.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` returning focus to the editor canvas
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// let _task = editor.open_command_palette();
    /// let _task = editor.close_command_palette();
    /// ```
    pub fn close_command_palette(&mut self) -> iced::Task<Message> {
        self.update(&Message::CloseCommandPalette)
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::dialog::MAX_VISIBLE_ROWS;
    use super::*;
    use crate::canvas_editor::compare_floats;
    use crate::{Language, Translations};

    fn labels(entries: &[PaletteEntry]) -> Vec<&str> {
        entries.iter().map(|entry| entry.label.as_str()).collect()
    }

    #[test]
    fn test_matches_query_accepts_out_of_order_free_subsequences() {
        assert!(matches_query("Toggle Line Comment", "tc"));
        assert!(matches_query("Fold All", "fldall"));
        assert!(matches_query("Fold All", "FOLD"));
        assert!(matches_query("Go to Line", ""));
        assert!(!matches_query("Fold All", "unfold"));
        assert!(!matches_query("Fold All", "lof"));
    }

    #[test]
    fn test_matches_query_is_case_insensitive_beyond_ascii() {
        assert!(matches_query("Aller à la ligne", "À"));
        assert!(matches_query("Tout replier", "TOUT"));
    }

    #[test]
    fn test_custom_entries_drop_disabled_items() {
        let entries = custom_entries(&[
            ContextMenuItem::new("app.format", "Format Document"),
            ContextMenuItem::new("app.rename", "Rename Symbol")
                .with_enabled(false),
        ]);

        assert_eq!(labels(&entries), vec!["Format Document"]);
        assert!(matches!(
            &entries[0].action,
            PaletteAction::Custom(id) if id == "app.format"
        ));
    }

    #[test]
    fn test_custom_entries_precede_built_in_entries() {
        let entries = build_entries(
            &[ContextMenuItem::new("app.format", "Format Document")],
            true,
            ActionContext::default(),
            &Translations::default(),
        );

        assert_eq!(entries[0].label, "Format Document");
        assert!(entries.len() > 1);
    }

    #[test]
    fn test_built_in_entries_can_be_turned_off() {
        let entries = build_entries(
            &[ContextMenuItem::new("app.format", "Format Document")],
            false,
            ActionContext::default(),
            &Translations::default(),
        );

        assert_eq!(labels(&entries), vec!["Format Document"]);
    }

    #[test]
    fn test_unavailable_actions_are_omitted() {
        let entries =
            default_entries(ActionContext::default(), &Translations::default());

        assert!(!labels(&entries).contains(&"Undo"));
        assert!(!labels(&entries).contains(&"Cut"));
        assert!(!labels(&entries).contains(&"Fold All"));
    }

    #[test]
    fn test_available_actions_are_listed() {
        let entries = default_entries(
            ActionContext {
                can_undo: true,
                can_redo: true,
                has_selection: true,
                has_content: true,
                reveal_in_file_manager_enabled: false,
                search_replace_enabled: true,
                folding_enabled: true,
            },
            &Translations::default(),
        );
        let labels = labels(&entries);

        assert!(labels.contains(&"Undo"));
        assert!(labels.contains(&"Redo"));
        assert!(labels.contains(&"Cut"));
        assert!(labels.contains(&"Select All"));
        assert!(labels.contains(&"Fold All"));
        assert!(labels.contains(&"Find"));
    }

    #[test]
    fn test_built_in_entries_use_the_selected_language() {
        let translations = Translations::new(Language::French);
        let entries = default_entries(ActionContext::default(), &translations);

        assert!(labels(&entries).contains(&"Aller à la ligne"));
        assert!(labels(&entries).contains(&"Coller"));
    }

    #[test]
    fn test_navigate_wraps_at_both_ends() {
        let mut state = CommandPaletteState::new();

        state.navigate(1, 3);
        assert_eq!(state.selected, 1);
        state.navigate(-1, 3);
        assert_eq!(state.selected, 0);
        state.navigate(-1, 3);
        assert_eq!(state.selected, 2);
        state.navigate(1, 3);
        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_navigate_on_empty_list_stays_at_zero() {
        let mut state = CommandPaletteState::new();
        state.selected = 4;

        state.navigate(1, 0);

        assert_eq!(state.selected, 0);
    }

    #[test]
    fn test_open_resets_the_previous_query_and_selection() {
        let mut state = CommandPaletteState::new();
        state.query = "fold".to_string();
        state.selected = 3;
        state.first_visible_row = 3;

        state.open();

        assert!(state.is_open);
        assert!(state.query.is_empty());
        assert_eq!(state.selected, 0);
        assert_eq!(state.first_visible_row, 0);
    }

    // ---- Scrolling the result list ----
    //
    // `window_origin` is the whole of the scroll arithmetic: the offset handed
    // to iced is `rows_to_pixels(first_visible_row)`, so a row index that is
    // right here is a pixel offset that is right there. `LONG` is longer than
    // one window, which is the only case where scrolling happens at all.

    /// A list comfortably longer than one window.
    const LONG: usize = MAX_VISIBLE_ROWS * 2 + 5;

    #[test]
    fn test_window_origin_does_not_move_for_a_row_already_visible() {
        // Every row of the first window, including its last, must leave the
        // list where it is — this is what the offset used to get wrong, by
        // scrolling the highlighted row up to the top on the very first Down.
        for selected in 0..MAX_VISIBLE_ROWS {
            assert_eq!(
                window_origin(selected, 0, LONG),
                0,
                "row {selected} is already visible in the first window"
            );
        }
    }

    #[test]
    fn test_window_origin_scrolls_by_one_row_when_the_highlight_moves_past_the_bottom()
     {
        assert_eq!(window_origin(MAX_VISIBLE_ROWS, 0, LONG), 1);
        assert_eq!(window_origin(MAX_VISIBLE_ROWS + 1, 1, LONG), 2);
    }

    #[test]
    fn test_window_origin_puts_a_row_above_the_window_at_the_top() {
        assert_eq!(window_origin(4, 7, LONG), 4);
    }

    #[test]
    fn test_window_origin_never_scrolls_past_the_last_full_window() {
        let last_origin = LONG - MAX_VISIBLE_ROWS;

        // Wrapping from the first row up to the last must show the final
        // window, not scroll the last row to the top and leave a gap below it.
        assert_eq!(window_origin(LONG - 1, 0, LONG), last_origin);
        // An origin left over from a longer list is clamped back in range.
        assert_eq!(window_origin(0, LONG, LONG), 0);
    }

    #[test]
    fn test_window_origin_stays_at_zero_for_a_list_that_fits() {
        for len in [0, 1, MAX_VISIBLE_ROWS] {
            for selected in 0..len {
                assert_eq!(
                    window_origin(selected, 0, len),
                    0,
                    "a list of {len} rows never scrolls"
                );
            }
        }
    }

    #[test]
    fn test_navigating_a_full_list_keeps_the_highlight_inside_the_window() {
        let mut state = CommandPaletteState::new();

        // One full lap down and back up: the highlight must never leave the
        // window, at any point, in either direction.
        for delta in [1, -1] {
            for _ in 0..=LONG {
                state.navigate(delta, LONG);
                let origin = state.first_visible_row;
                assert!(
                    (origin..origin + MAX_VISIBLE_ROWS)
                        .contains(&state.selected),
                    "row {} is outside the window at {origin}",
                    state.selected
                );
                assert!(
                    origin <= LONG - MAX_VISIBLE_ROWS,
                    "the window starts at {origin}, past the end of the list"
                );
            }
        }
    }

    #[test]
    fn test_row_offsets_account_for_the_gap_between_rows() {
        // A row's offset is its pitch, not its height: the two differ by the
        // one-pixel gap the list puts between rows, and a scroll computed
        // from the height alone drifts by one pixel per row.
        let two_rows = dialog::rows_to_pixels(2);
        let one_row = dialog::rows_to_pixels(1);

        assert_eq!(
            compare_floats(dialog::rows_to_pixels(0), 0.0),
            Ordering::Equal
        );
        assert_eq!(
            compare_floats(two_rows - one_row, one_row),
            Ordering::Equal,
            "rows must be evenly pitched"
        );
        assert!(
            one_row > 26.0,
            "the pitch must exceed the 26px row height by the row gap"
        );
    }
}
