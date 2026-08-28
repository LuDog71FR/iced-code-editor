//! Canvas-based text editor widget for maximum performance.
//!
//! This module provides a custom Canvas widget that handles all text rendering
//! and input directly, bypassing Iced's higher-level widgets for optimal speed.

use iced::widget::operation::{RelativeOffset, snap_to};
use iced::widget::{Id, canvas};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

use crate::buffer::TextBuffer;
use crate::i18n::Translations;
use crate::theme::Style;
use caches::{
    BracketDepthCache, HighlightCache, MaxContentWidthCache, ResolvedSyntax,
    VisualLinesCache,
};
use editing::cursor_set;
pub use editing::history::CommandHistory;
use features::{command_palette, folding, goto_line, search, vim};
use metrics::{
    CACHE_WINDOW_MARGIN_MULTIPLIER, CHAR_WIDTH, CURSOR_BLINK_INTERVAL,
    FONT_SIZE, GUTTER_WIDTH, HIGHLIGHT_LINES_PER_FRAME, LINE_HEIGHT, TAB_WIDTH,
    compare_floats, indent_width, measure_char_width, measure_text_width,
};

#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Global counter for generating unique editor IDs (starts at 1)
static EDITOR_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// ID of the currently focused editor (0 = no editor focused)
static FOCUSED_EDITOR_ID: AtomicU64 = AtomicU64::new(0);

// Re-export submodules
#[cfg(feature = "bench")]
#[doc(hidden)]
pub mod bench_support;
mod caches;
mod config;
mod editing;
pub(crate) mod features;
mod focus;
mod input;
pub mod lsp;
mod metrics;
mod render;

pub use features::context_menu::{ContextMenuEntry, ContextMenuItem};
pub use features::vim::VimMode;
#[derive(Debug, Clone)]
pub(crate) struct ImePreedit {
    pub(crate) content: String,
    pub(crate) selection: Option<Range<usize>>,
}

/// Conservative logical-line range captured immediately before an edit.
///
/// It lets LSP synchronization send one incremental range replacement without
/// serializing and diffing the entire document after every keystroke.
pub(crate) struct LspEditSnapshot {
    pub(crate) start_line: usize,
    pub(crate) old_end_exclusive: usize,
    pub(crate) old_line_count: usize,
    pub(crate) old_end: lsp::LspPosition,
}

/// Canvas-based high-performance text editor.
///
/// The editor is a self-contained Iced widget: build one with [`Self::new`],
/// render it with [`Self::view`], and feed the [`Message`]s it emits back into
/// [`Self::update`]. Everything else — syntax highlighting, multi-cursor,
/// search/replace, folding, Vim mode, LSP synchronization — is configured
/// through the builder methods and setters on this type.
///
/// # Examples
///
/// ```
/// use iced_code_editor::{CodeEditor, Message};
///
/// // Configure with the builder methods...
/// let mut editor = CodeEditor::new("fn main() {}", "rs")
///     .with_wrap_enabled(false)
///     .with_line_numbers_enabled(true);
///
/// // ...then drive it with messages, as the host's `update` would.
/// let _ = editor.update(&Message::Paste("// hello\n".to_string()));
/// assert_eq!(editor.content(), "// hello\nfn main() {}");
/// assert!(editor.is_modified());
/// ```
pub struct CodeEditor {
    /// Unique ID for this editor instance (for focus management)
    pub(crate) editor_id: u64,
    /// Text buffer
    pub(crate) buffer: TextBuffer,
    /// All cursor positions (multi-cursor support).
    pub(crate) cursors: cursor_set::CursorSet,
    /// Horizontal scroll offset in pixels, only used when wrap_enabled = false
    pub(crate) horizontal_scroll_offset: f32,
    /// Editor theme style
    pub(crate) style: Style,
    /// Syntax highlighting language
    pub(crate) syntax: String,
    /// Last cursor blink time
    pub(crate) last_blink: Instant,
    /// Cursor visible state
    pub(crate) cursor_visible: bool,
    /// Mouse is currently dragging for selection
    pub(crate) is_dragging: bool,
    /// Cached geometry for the "content" layer.
    ///
    /// This layer includes expensive-to-build, mostly static visuals such as:
    /// - syntax-highlighted text glyphs
    /// - line numbers / gutter text
    ///
    /// It is intentionally kept stable across selection/cursor movement so
    /// that mouse-drag selection feels smooth.
    pub(crate) content_cache: canvas::Cache,
    /// Cached geometry for the "overlay" layer.
    ///
    /// This layer includes visuals that change frequently without modifying the
    /// underlying buffer, such as:
    /// - cursor and current-line highlight
    /// - selection highlight
    /// - search match highlights
    /// - IME preedit decorations
    ///
    /// Keeping overlays in a separate cache avoids invalidating the content
    /// layer on every cursor blink or selection drag.
    pub(crate) overlay_cache: canvas::Cache,
    /// Scrollable ID for programmatic scrolling
    pub(crate) scrollable_id: Id,
    /// ID for the horizontal scrollable widget (only used when wrap_enabled = false)
    pub(crate) horizontal_scrollable_id: Id,
    /// Incremental per-line width index for the horizontal scrollbar.
    pub(crate) max_content_width_cache: RefCell<Option<MaxContentWidthCache>>,
    /// Current viewport scroll position (Y offset)
    pub(crate) viewport_scroll: f32,
    /// Viewport height (visible area)
    pub(crate) viewport_height: f32,
    /// Viewport width (visible area)
    pub(crate) viewport_width: f32,
    /// Command history for undo/redo
    pub(crate) history: CommandHistory,
    /// Whether we're currently grouping commands (for smart undo)
    pub(crate) is_grouping: bool,
    /// Line wrapping enabled
    pub(crate) wrap_enabled: bool,
    /// Auto-indentation enabled
    pub(crate) auto_indent_enabled: bool,
    /// Auto-closing of brackets/quotes and surround-selection enabled
    pub(crate) auto_close_brackets: bool,
    /// Indentation style (spaces or tab)
    pub(crate) indent_style: IndentStyle,
    /// Wrap column (None = wrap at viewport width)
    pub(crate) wrap_column: Option<usize>,
    /// Whether code folding (collapse/expand blocks) is enabled.
    pub(crate) folding_enabled: bool,
    /// Header line indices of regions that are currently collapsed.
    pub(crate) collapsed_folds: HashSet<usize>,
    /// Monotonic revision counter for fold state.
    ///
    /// Bumped whenever the collapsed set or the folding toggle changes, so that
    /// derived layout caches (visual lines) are invalidated.
    pub(crate) fold_revision: u64,
    /// Cached foldable regions, keyed by `buffer_revision`.
    pub(crate) foldable_regions_cache:
        RefCell<Option<(u64, Rc<Vec<folding::FoldRegion>>)>>,
    /// Search state
    pub(crate) search_state: search::SearchState,
    /// Custom entries displayed before the built-in context-menu actions.
    custom_context_menu_entries: Vec<ContextMenuEntry>,
    /// Whether the built-in editing actions are shown in the context menu.
    default_context_menu_enabled: bool,
    /// Whether the built-in reveal-in-file-manager action is shown.
    reveal_in_file_manager_enabled: bool,
    /// Go-to-line dialog state
    pub(crate) goto_line_state: goto_line::GotoLineState,
    /// Command palette state
    pub(crate) command_palette_state: command_palette::CommandPaletteState,
    /// Custom commands listed before the built-in ones in the palette.
    custom_command_palette_entries: Vec<ContextMenuItem>,
    /// Whether the built-in editor commands are listed in the palette.
    default_command_palette_enabled: bool,
    /// Whether the command palette can be opened at all.
    pub(crate) command_palette_enabled: bool,
    /// Whether Vim key handling is enabled for this editor instance.
    vim_enabled: bool,
    /// Per-editor Vim mode, parser prefixes and unnamed register.
    pub(crate) vim_state: vim::VimState,
    /// Translations for UI text
    pub(crate) translations: Translations,
    /// Whether search/replace functionality is enabled
    pub(crate) search_replace_enabled: bool,
    /// Whether line numbers are displayed
    pub(crate) line_numbers_enabled: bool,
    /// Whether to render whitespace characters visibly (spaces as `·`, tabs as `→`)
    pub(crate) show_whitespace: bool,
    /// Whether vertical indentation guides are drawn behind the text.
    pub(crate) show_indent_guides: bool,
    /// Whether inline color-preview swatches are drawn next to color literals.
    pub(crate) show_color_previews: bool,
    /// Whether the matching-bracket/quote-pair highlight overlay is enabled.
    pub(crate) bracket_match_highlight_enabled: bool,
    /// Whether bracket-pair colorization (rainbow brackets) is enabled.
    pub(crate) bracket_pair_colorization_enabled: bool,
    /// Whether the headers of the enclosing blocks are pinned above the viewport.
    pub(crate) sticky_scroll_enabled: bool,
    /// Whether LSP support is enabled
    pub(crate) lsp_enabled: bool,
    /// Active LSP client connection, if configured.
    pub(crate) lsp_client: Option<Box<dyn lsp::LspClient>>,
    /// Metadata for the currently open LSP document.
    pub(crate) lsp_document: Option<lsp::LspDocument>,
    /// Pending incremental LSP text changes not yet flushed.
    pub(crate) lsp_pending_changes: Vec<lsp::LspTextChange>,
    /// Shadow copy of buffer content used to compute LSP deltas.
    pub(crate) lsp_shadow_text: String,
    /// Whether `lsp_shadow_text` still exactly matches the server document.
    pub(crate) lsp_shadow_is_current: bool,
    /// Current server-side line count, maintained incrementally.
    pub(crate) lsp_synced_line_count: usize,
    /// Length of the current server-side final line in Unicode scalar values.
    pub(crate) lsp_synced_last_line_len: usize,
    /// Pre-edit range used to build a bounded incremental LSP change.
    pub(crate) lsp_edit_snapshot: Option<LspEditSnapshot>,
    /// Whether to auto-flush LSP changes after edits.
    pub(crate) lsp_auto_flush: bool,
    /// Whether the canvas has user input focus (for keyboard events)
    pub(crate) has_canvas_focus: bool,
    /// Whether input processing is locked to prevent focus stealing
    pub(crate) focus_locked: bool,
    /// Whether to show the cursor (for rendering)
    pub(crate) show_cursor: bool,
    /// Current keyboard modifiers state (Ctrl, Alt, Shift, Logo).
    ///
    /// This is updated via subscription events and used to handle modifier-dependent
    /// interactions, such as "Ctrl+Click" for jumping to a definition.
    pub(crate) modifiers: Cell<iced::keyboard::Modifiers>,
    /// Last left-button press (time, position, consecutive count), used to
    /// detect double/triple clicks.
    pub(crate) last_click: Cell<Option<(Instant, iced::Point, u8)>>,
    /// The font used for rendering text
    pub(crate) font: iced::Font,
    /// IME pre-edit state (for CJK input)
    pub(crate) ime_preedit: Option<ImePreedit>,
    /// Font size in pixels
    pub(crate) font_size: f32,
    /// Full character width (wide chars like CJK) in pixels
    pub(crate) full_char_width: f32,
    /// Line height in pixels
    pub(crate) line_height: f32,
    /// Character width in pixels
    pub(crate) char_width: f32,
    /// Cached render window: the first visual line index included in the cache.
    /// We keep a larger window than the currently visible range to avoid clearing
    /// the canvas cache on every small scroll. Only when scrolling crosses the
    /// window boundary do we re-window and clear the cache.
    pub(crate) last_first_visible_line: usize,
    /// Cached render window start line (inclusive)
    pub(crate) cache_window_start_line: usize,
    /// Cached render window end line (exclusive)
    pub(crate) cache_window_end_line: usize,
    /// Monotonic revision counter for buffer content.
    ///
    /// Any operation that changes the buffer must bump this counter to
    /// invalidate derived layout caches (e.g. wrapping / visual lines). The
    /// exact value is not semantically meaningful, so `wrapping_add` is used to
    /// avoid overflow panics while still producing a different key.
    pub(crate) buffer_revision: u64,
    /// Cached result of line wrapping ("visual lines") for the current layout key.
    ///
    /// This is stored behind a `RefCell` because wrapping is needed during
    /// rendering (where we only have `&self`), but we still want to memoize the
    /// expensive computation without forcing external mutability.
    visual_lines_cache: RefCell<Option<VisualLinesCache>>,
    /// Memoized syntax/theme resolution (see [`ResolvedSyntax`]).
    ///
    /// Behind a `RefCell` for the same reason as the caches below: resolution
    /// happens during rendering, where only `&self` is available. Self-keyed on
    /// the syntax identifier and the background lightness, so it needs no
    /// explicit invalidation from `set_syntax`/`set_theme`.
    pub(crate) resolved_syntax: RefCell<Option<ResolvedSyntax>>,
    /// Sequential per-line syntax-highlight cache (see [`HighlightCache`]).
    ///
    /// Stored behind a `RefCell` because highlighting is performed during
    /// rendering (where only `&self` is available) yet should be memoized.
    /// Spans are reused across wrapped visual segments and across scroll-only
    /// renders. On an edit the cache is truncated from the first changed line
    /// (tracked via `pre_edit_line`) rather than fully cleared, so multi-line
    /// constructs stay correct without re-parsing the whole file.
    pub(crate) highlight_cache: RefCell<Option<HighlightCache>>,
    /// Remaining syntax lines that may be parsed during the current content
    /// render. `usize::MAX` keeps direct non-render uses (notably tests) uncapped.
    pub(crate) highlight_lines_remaining: Cell<usize>,
    /// Sequential per-line bracket-nesting-depth cache (see [`BracketDepthCache`]),
    /// used by bracket-pair colorization. Stored behind a `RefCell` for the same
    /// reason as `highlight_cache`: it is memoized during rendering (`&self`).
    pub(crate) bracket_depth_cache: RefCell<BracketDepthCache>,
    /// Topmost logical line touched by the cursors/selections before the
    /// current edit, captured at the top of `update()`.
    ///
    /// Used as a conservative lower bound for the first line an edit may
    /// change, to truncate `highlight_cache` precisely.
    pub(crate) pre_edit_line: usize,
    /// Bottommost logical line touched before the current edit.
    ///
    /// Together with `pre_edit_line`, this bounds the portion of the visual-line
    /// cache that must be rebuilt after a localized edit.
    pub(crate) pre_edit_last_line: usize,
}

/// Messages emitted by the code editor
///
/// A host application forwards these to [`CodeEditor::update`]. They are also
/// the programmatic control surface: sending a variant directly performs the
/// same action as the user gesture that would normally produce it, which is
/// how a host wires its own menu items and toolbar buttons to editor commands.
///
/// # Examples
///
/// ```
/// use iced_code_editor::{CodeEditor, Message};
///
/// let mut editor = CodeEditor::new("hello", "rs");
///
/// // Drive an edit as if the user had pasted.
/// let _ = editor.update(&Message::Paste(" world".to_string()));
/// assert_eq!(editor.content(), " worldhello");
///
/// // And undo it the same way a menu item would.
/// let _ = editor.update(&Message::Undo);
/// assert_eq!(editor.content(), "hello");
/// ```
#[derive(Debug, Clone)]
pub enum Message {
    /// Character typed
    CharacterInput(char),
    /// A printable key interpreted by the Vim state machine.
    VimKey(char),
    /// Toggle Vim behavior for this editor instance.
    ToggleVimMode,
    /// Requests that the host save this editor's current document.
    WriteRequested,
    /// Backspace pressed
    Backspace,
    /// Delete pressed
    Delete,
    /// Enter pressed
    Enter,
    /// Tab pressed (inserts 4 spaces)
    Tab,
    /// Arrow key pressed (direction, shift_pressed)
    ArrowKey(ArrowDirection, bool),
    /// Mouse clicked at position
    MouseClick(iced::Point),
    /// Mouse drag for selection
    MouseDrag(iced::Point),
    /// Mouse moved within the editor without dragging
    MouseHover(iced::Point),
    /// Mouse released
    MouseRelease,
    /// Double-click: select the word under the cursor
    DoubleClick(iced::Point),
    /// Triple-click: select the whole line under the cursor
    TripleClick(iced::Point),
    /// Right-clicked in the editor to position and open the context menu
    ContextMenuRequested(iced::Point),
    /// A configured context-menu action was selected.
    CustomContextMenuAction(String),
    /// Requests that the host reveal the editor's file in the system file manager.
    RevealInFileManager,
    /// Cut selected text
    Cut,
    /// Copy selected text (Ctrl+C)
    Copy,
    /// Paste text from clipboard (Ctrl+V)
    Paste(String),
    /// Delete selected text (Shift+Delete)
    DeleteSelection,
    /// Select the complete document
    SelectAll,
    /// Request redraw for cursor blink
    Tick,
    /// Page Up pressed
    PageUp,
    /// Page Down pressed
    PageDown,
    /// Home key pressed (move to start of line, shift_pressed)
    Home(bool),
    /// End key pressed (move to end of line, shift_pressed)
    End(bool),
    /// Ctrl+Home pressed (move to start of document)
    CtrlHome,
    /// Ctrl+End pressed (move to end of document)
    CtrlEnd,
    /// Go to an explicit logical position (line, column), both 0-based.
    GotoPosition(usize, usize),
    /// Open the go-to-line dialog (Cmd/Ctrl+G).
    OpenGotoLine,
    /// Close the go-to-line dialog.
    CloseGotoLine,
    /// Change the one-based line number shown in the go-to-line input.
    GotoLineChanged(String),
    /// Submit the current go-to-line input.
    SubmitGotoLine,
    /// Open the command palette (Cmd/Ctrl+Shift+P).
    OpenCommandPalette,
    /// Close the command palette without running anything.
    CloseCommandPalette,
    /// Change the filter text typed in the command palette.
    CommandPaletteChanged(String),
    /// Move the command palette highlight one row down (`true`) or up.
    CommandPaletteNavigate(bool),
    /// Highlight the command palette row at this index and run it.
    CommandPaletteSelected(usize),
    /// Run the currently highlighted command palette row.
    SubmitCommandPalette,
    /// A host-registered command palette entry was run, identified by the
    /// `id` it was registered with. The editor never acts on this itself:
    /// handle it in the host application.
    CommandPaletteAction(String),
    /// Viewport scrolled - track scroll position
    Scrolled(iced::widget::scrollable::Viewport),
    /// Horizontal scrollbar scrolled (only when wrap is disabled)
    HorizontalScrolled(iced::widget::scrollable::Viewport),
    /// Undo last operation (Ctrl+Z)
    Undo,
    /// Redo last undone operation (Ctrl+Y)
    Redo,
    /// Open search dialog (Ctrl+F)
    OpenSearch,
    /// Open search and replace dialog (Ctrl+H)
    OpenSearchReplace,
    /// Close search dialog (Escape)
    CloseSearch,
    /// Search query text changed
    SearchQueryChanged(String),
    /// Replace text changed
    ReplaceQueryChanged(String),
    /// Toggle case sensitivity
    ToggleCaseSensitive,
    /// Find next match (F3)
    FindNext,
    /// Find previous match (Shift+F3)
    FindPrevious,
    /// Replace current match
    ReplaceNext,
    /// Replace all matches
    ReplaceAll,
    /// Tab pressed in search dialog (cycle forward)
    SearchDialogTab,
    /// Shift+Tab pressed in search dialog (cycle backward)
    SearchDialogShiftTab,
    /// Shift+Tab pressed for focus navigation (when search dialog is not open)
    FocusNavigationShiftTab,
    /// Canvas gained focus (mouse click)
    CanvasFocusGained,
    /// Canvas lost focus (external widget interaction)
    CanvasFocusLost,
    /// Triggered when the user performs a Ctrl+Click (or Cmd+Click on macOS)
    /// on the editor content, intending to jump to the definition of the symbol
    /// under the cursor.
    JumpClick(iced::Point),
    /// IME input method opened
    ImeOpened,
    /// IME pre-edit update (content, selection range)
    ImePreedit(String, Option<Range<usize>>),
    /// IME commit text
    ImeCommit(String),
    /// IME input method closed
    ImeClosed,
    /// Alt+Click: add a new cursor at the given canvas position
    AltClick(iced::Point),
    /// Ctrl+Alt+Up: add a cursor on the line above the primary cursor
    AddCursorAbove,
    /// Ctrl+Alt+Down: add a cursor on the line below the primary cursor
    AddCursorBelow,
    /// Ctrl+D: select the next occurrence of the currently selected text (or word under cursor)
    SelectNextOccurrence,
    /// Toggle the collapsed state of the fold whose header is the given logical line.
    ToggleFold(usize),
    /// Toggle the collapsed state of the innermost block containing the primary cursor.
    ToggleFoldAtCursor,
    /// Fold every foldable block in the buffer.
    FoldAll,
    /// Unfold every collapsed block in the buffer.
    UnfoldAll,
    /// Alt+Up: move the current line (or selected line range) up by one line.
    MoveLineUp,
    /// Alt+Down: move the current line (or selected line range) down by one line.
    MoveLineDown,
    /// Shift+Alt+Up: duplicate the current line (or selected line range) above.
    DuplicateLineUp,
    /// Shift+Alt+Down: duplicate the current line (or selected line range) below.
    DuplicateLineDown,
    /// Ctrl+/: toggle line comments on the current line or primary selection.
    ToggleComment,
    /// Scroll back to the given logical line, whose sticky-scroll header was clicked.
    StickyScrollJump(usize),
}

/// Indentation style used when pressing the Tab key.
///
/// Controls whether indentation inserts spaces or a tab character.
///
/// Implements [`Display`](std::fmt::Display) with a human-readable label
/// (`"4 spaces"`, `"Tab"`), so it can be dropped straight into a settings
/// picker.
///
/// # Examples
///
/// ```
/// use iced_code_editor::{CodeEditor, IndentStyle};
///
/// let mut editor = CodeEditor::new("fn main() {}", "rs");
/// assert_eq!(editor.indent_style(), IndentStyle::Spaces(4));
///
/// editor.set_indent_style(IndentStyle::Tab);
/// assert_eq!(editor.indent_style().to_string(), "Tab");
/// assert_eq!(IndentStyle::Spaces(2).to_string(), "2 spaces");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentStyle {
    /// Insert `n` space characters.
    Spaces(u8),
    /// Insert a single tab character (`\t`).
    Tab,
}

impl IndentStyle {
    /// All standard indentation styles available for selection.
    ///
    /// Iterate this to build a settings picker, rather than hardcoding the
    /// list at each call site.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::IndentStyle;
    ///
    /// let labels: Vec<String> =
    ///     IndentStyle::ALL.iter().map(ToString::to_string).collect();
    /// assert_eq!(labels, ["2 spaces", "4 spaces", "8 spaces", "Tab"]);
    /// ```
    pub const ALL: [IndentStyle; 4] = [
        IndentStyle::Spaces(2),
        IndentStyle::Spaces(4),
        IndentStyle::Spaces(8),
        IndentStyle::Tab,
    ];
}

impl std::fmt::Display for IndentStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndentStyle::Spaces(1) => write!(f, "1 space"),
            IndentStyle::Spaces(n) => write!(f, "{n} spaces"),
            IndentStyle::Tab => write!(f, "Tab"),
        }
    }
}

/// Arrow key directions
///
/// Carried by [`Message::ArrowKey`] alongside a flag for whether Shift was
/// held, which is what turns a move into a selection.
///
/// # Examples
///
/// ```
/// use iced_code_editor::{ArrowDirection, CodeEditor, Message};
///
/// let mut editor = CodeEditor::new("hello", "rs");
///
/// // Move right without extending a selection.
/// let _ = editor.update(&Message::ArrowKey(ArrowDirection::Right, false));
/// ```
#[derive(Debug, Clone, Copy)]
pub enum ArrowDirection {
    /// Up arrow key.
    Up,
    /// Down arrow key.
    Down,
    /// Left arrow key.
    Left,
    /// Right arrow key.
    Right,
}

impl CodeEditor {
    /// Creates a new canvas-based text editor.
    ///
    /// # Arguments
    ///
    /// * `content` - Initial text content
    /// * `syntax` - Syntax highlighting language (e.g., "py", "lua", "rs")
    ///
    /// # Returns
    ///
    /// A new `CodeEditor` instance
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert_eq!(editor.content(), "fn main() {}");
    /// assert_eq!(editor.syntax(), "rs");
    ///
    /// // A new editor starts clean, with nothing to undo.
    /// assert!(!editor.is_modified());
    /// assert!(!editor.can_undo());
    /// ```
    pub fn new(content: &str, syntax: &str) -> Self {
        // Generate a unique ID for this editor instance
        let editor_id = EDITOR_ID_COUNTER.fetch_add(1, Ordering::Relaxed);

        // Give focus to the first editor created (ID == 1)
        if editor_id == 1 {
            FOCUSED_EDITOR_ID.store(editor_id, Ordering::Relaxed);
        }

        let mut editor = Self {
            editor_id,
            buffer: TextBuffer::new(content),
            cursors: cursor_set::CursorSet::new((0, 0)),
            horizontal_scroll_offset: 0.0,
            style: crate::theme::from_iced_theme(&iced::Theme::TokyoNightStorm),
            syntax: syntax.to_string(),
            last_blink: Instant::now(),
            cursor_visible: true,
            is_dragging: false,
            content_cache: canvas::Cache::default(),
            overlay_cache: canvas::Cache::default(),
            scrollable_id: Id::unique(),
            horizontal_scrollable_id: Id::unique(),
            max_content_width_cache: RefCell::new(None),
            viewport_scroll: 0.0,
            viewport_height: 600.0, // Default, will be updated
            viewport_width: 800.0,  // Default, will be updated
            history: CommandHistory::new(100),
            is_grouping: false,
            wrap_enabled: true,
            auto_indent_enabled: true,
            auto_close_brackets: true,
            indent_style: IndentStyle::Spaces(4),
            wrap_column: None,
            folding_enabled: true,
            collapsed_folds: HashSet::new(),
            fold_revision: 0,
            foldable_regions_cache: RefCell::new(None),
            search_state: search::SearchState::new(),
            custom_context_menu_entries: Vec::new(),
            default_context_menu_enabled: true,
            reveal_in_file_manager_enabled: false,
            goto_line_state: goto_line::GotoLineState::new(),
            command_palette_state: command_palette::CommandPaletteState::new(),
            custom_command_palette_entries: Vec::new(),
            default_command_palette_enabled: true,
            command_palette_enabled: true,
            vim_enabled: false,
            vim_state: vim::VimState::default(),
            translations: Translations::default(),
            search_replace_enabled: true,
            line_numbers_enabled: true,
            show_whitespace: true,
            show_indent_guides: true,
            show_color_previews: true,
            bracket_match_highlight_enabled: true,
            bracket_pair_colorization_enabled: true,
            sticky_scroll_enabled: true,
            lsp_enabled: true,
            lsp_client: None,
            lsp_document: None,
            lsp_pending_changes: Vec::new(),
            lsp_shadow_text: String::new(),
            lsp_shadow_is_current: true,
            lsp_synced_line_count: 1,
            lsp_synced_last_line_len: 0,
            lsp_edit_snapshot: None,
            lsp_auto_flush: true,
            has_canvas_focus: false,
            focus_locked: false,
            show_cursor: false,
            modifiers: Cell::new(iced::keyboard::Modifiers::default()),
            last_click: Cell::new(None),
            font: iced::Font::MONOSPACE,
            ime_preedit: None,
            font_size: FONT_SIZE,
            full_char_width: CHAR_WIDTH * 2.0,
            line_height: LINE_HEIGHT,
            char_width: CHAR_WIDTH,
            // Initialize render window tracking for virtual scrolling:
            // these indices define the cached visual line window. The window is
            // expanded beyond the visible range to amortize redraws and keep scrolling smooth.
            last_first_visible_line: 0,
            cache_window_start_line: 0,
            cache_window_end_line: 0,
            buffer_revision: 0,
            visual_lines_cache: RefCell::new(None),
            resolved_syntax: RefCell::new(None),
            highlight_cache: RefCell::new(None),
            highlight_lines_remaining: Cell::new(usize::MAX),
            bracket_depth_cache: RefCell::new(BracketDepthCache::new()),
            pre_edit_line: 0,
            pre_edit_last_line: 0,
        };

        // Perform initial character dimension calculation
        editor.recalculate_char_dimensions(false);

        editor
    }

    /// Returns the current text content as a string.
    ///
    /// The buffer's original line endings and trailing newline are preserved,
    /// so the result round-trips back to disk unchanged when nothing was
    /// edited.
    ///
    /// This allocates the whole document, so prefer calling it when saving
    /// rather than on every frame.
    ///
    /// # Returns
    ///
    /// The complete text content of the editor
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// assert_eq!(editor.content(), "fn main() {}");
    ///
    /// let _ = editor.update(&Message::Paste("// note\n".to_string()));
    /// assert_eq!(editor.content(), "// note\nfn main() {}");
    /// ```
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }

    /// Resets the editor with new content.
    ///
    /// This method replaces the buffer content and resets all editor state
    /// (cursor position, selection, scroll, history) to initial values.
    /// Use this instead of creating a new `CodeEditor` instance to ensure
    /// proper widget tree updates in iced.
    ///
    /// Returns a `Task` that scrolls the editor to the top, which also
    /// forces a redraw of the canvas.
    ///
    /// # Arguments
    ///
    /// * `content` - The new text content
    ///
    /// # Returns
    ///
    /// A `Task<Message>` that should be returned from your update function
    ///
    /// # Example
    ///
    /// ```no_run
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("initial content", "lua");
    /// // Later, reset with new content and get the task
    /// let task = editor.reset("new content");
    /// // Return task.map(YourMessage::Editor) from your update function
    /// ```
    pub fn reset(&mut self, content: &str) -> iced::Task<Message> {
        self.buffer = TextBuffer::new(content);
        self.cursors.set_single((0, 0));
        self.vim_state.reset();
        self.horizontal_scroll_offset = 0.0;
        self.is_dragging = false;
        self.viewport_scroll = 0.0;
        self.history = CommandHistory::new(100);
        self.is_grouping = false;
        self.last_blink = Instant::now();
        self.cursor_visible = true;
        self.content_cache = canvas::Cache::default();
        self.overlay_cache = canvas::Cache::default();
        self.buffer_revision = self.buffer_revision.wrapping_add(1);
        *self.visual_lines_cache.borrow_mut() = None;
        // The buffer is fully replaced, so discard the whole highlight prefix.
        self.pre_edit_line = 0;
        self.pre_edit_last_line = usize::MAX;
        self.invalidate_highlight_from(0);
        *self.bracket_depth_cache.borrow_mut() = BracketDepthCache::new();
        self.enqueue_lsp_change();

        // Scroll to top to force a redraw
        snap_to(self.scrollable_id.clone(), RelativeOffset::START)
    }

    /// Resets the cursor blink animation.
    pub(crate) fn reset_cursor_blink(&mut self) {
        self.last_blink = Instant::now();
        self.cursor_visible = true;
    }
}
