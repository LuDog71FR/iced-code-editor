//! Canvas-based text editor widget for maximum performance.
//!
//! This module provides a custom Canvas widget that handles all text rendering
//! and input directly, bypassing Iced's higher-level widgets for optimal speed.

use iced::advanced::text::{
    Alignment, Paragraph, Renderer as TextRenderer, Text,
};
use iced::widget::operation::{RelativeOffset, snap_to};
use iced::widget::{Id, canvas};
use std::cmp::Ordering as CmpOrdering;
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use unicode_width::UnicodeWidthChar;

use crate::i18n::Translations;
use crate::text_buffer::TextBuffer;
use crate::theme::Style;
pub use history::CommandHistory;

/// Global counter for generating unique editor IDs (starts at 1)
static EDITOR_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

/// ID of the currently focused editor (0 = no editor focused)
static FOCUSED_EDITOR_ID: AtomicU64 = AtomicU64::new(0);

// Re-export submodules
mod canvas_impl;
mod clipboard;
pub mod command;
mod cursor;
pub mod history;
pub mod ime_requester;
mod search;
mod search_dialog;
mod selection;
mod update;
mod view;
mod wrapping;

/// Canvas-based text editor constants
pub(crate) const FONT_SIZE: f32 = 14.0;
pub(crate) const LINE_HEIGHT: f32 = 20.0;
pub(crate) const CHAR_WIDTH: f32 = 8.4; // Monospace character width
pub(crate) const GUTTER_WIDTH: f32 = 45.0;
pub(crate) const CURSOR_BLINK_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(530);

/// Measures the width of a single character.
///
/// # Arguments
///
/// * `c` - The character to measure
/// * `full_char_width` - The width of a full-width character
/// * `char_width` - The width of the character
///
/// # Returns
///
/// The calculated width of the character as a `f32`
pub(crate) fn measure_char_width(
    c: char,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    match c.width() {
        Some(w) if w > 1 => full_char_width,
        Some(_) => char_width,
        None => 0.0,
    }
}

/// Measures rendered text width, accounting for CJK wide characters.
///
/// - Wide characters (e.g. Chinese) use FONT_SIZE.
/// - Narrow characters (e.g. Latin) use CHAR_WIDTH.
/// - Control characters have width 0.
///
/// # Arguments
///
/// * `text` - The text string to measure
/// * `full_char_width` - The width of a full-width character
/// * `char_width` - The width of a regular character
///
/// # Returns
///
/// The total calculated width of the text as a `f32`
pub(crate) fn measure_text_width(
    text: &str,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    text.chars()
        .map(|c| measure_char_width(c, full_char_width, char_width))
        .sum()
}

/// Epsilon value for floating-point comparisons in text layout.
pub(crate) const EPSILON: f32 = 0.001;

/// Compares two floating point numbers with a small epsilon tolerance.
///
/// # Arguments
///
/// * `a` - first float number
/// * `b` - second float number
///
/// # Returns
///
/// * `Ordering::Equal` if `abs(a - b) < EPSILON`
/// * `Ordering::Greater` if `a > b` (and not equal)
/// * `Ordering::Less` if `a < b` (and not equal)
pub(crate) fn compare_floats(a: f32, b: f32) -> CmpOrdering {
    if (a - b).abs() < EPSILON {
        CmpOrdering::Equal
    } else if a > b {
        CmpOrdering::Greater
    } else {
        CmpOrdering::Less
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ImePreedit {
    pub(crate) content: String,
    pub(crate) selection: Option<Range<usize>>,
}

/// Canvas-based high-performance text editor.
pub struct CodeEditor {
    /// Unique ID for this editor instance (for focus management)
    pub(crate) editor_id: u64,
    /// Text buffer
    pub(crate) buffer: TextBuffer,
    /// Cursor position (line, column)
    pub(crate) cursor: (usize, usize),
    /// Scroll offset in pixels
    pub(crate) scroll_offset: f32,
    /// Editor theme style
    pub(crate) style: Style,
    /// Syntax highlighting language
    pub(crate) syntax: String,
    /// Last cursor blink time
    pub(crate) last_blink: Instant,
    /// Cursor visible state
    pub(crate) cursor_visible: bool,
    /// Selection start (if any)
    pub(crate) selection_start: Option<(usize, usize)>,
    /// Selection end (if any) - cursor position during selection
    pub(crate) selection_end: Option<(usize, usize)>,
    /// Mouse is currently dragging for selection
    pub(crate) is_dragging: bool,
    /// Cache for canvas rendering
    pub(crate) cache: canvas::Cache,
    /// Scrollable ID for programmatic scrolling
    pub(crate) scrollable_id: Id,
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
    /// Wrap column (None = wrap at viewport width)
    pub(crate) wrap_column: Option<usize>,
    /// Search state
    pub(crate) search_state: search::SearchState,
    /// Translations for UI text
    pub(crate) translations: Translations,
    /// Whether search/replace functionality is enabled
    pub(crate) search_replace_enabled: bool,
    /// Whether line numbers are displayed
    pub(crate) line_numbers_enabled: bool,
    /// Whether the canvas has user input focus (for keyboard events)
    pub(crate) has_canvas_focus: bool,
    /// Whether to show the cursor (for rendering)
    pub(crate) show_cursor: bool,
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
}

/// Messages emitted by the code editor
#[derive(Debug, Clone)]
pub enum Message {
    /// Character typed
    CharacterInput(char),
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
    /// Mouse released
    MouseRelease,
    /// Copy selected text (Ctrl+C)
    Copy,
    /// Paste text from clipboard (Ctrl+V)
    Paste(String),
    /// Delete selected text (Shift+Delete)
    DeleteSelection,
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
    /// Viewport scrolled - track scroll position
    Scrolled(iced::widget::scrollable::Viewport),
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
    /// Canvas gained focus (mouse click)
    CanvasFocusGained,
    /// Canvas lost focus (external widget interaction)
    CanvasFocusLost,
    /// IME input method opened
    ImeOpened,
    /// IME pre-edit update (content, selection range)
    ImePreedit(String, Option<Range<usize>>),
    /// IME commit text
    ImeCommit(String),
    /// IME input method closed
    ImeClosed,
}

/// Arrow key directions
#[derive(Debug, Clone, Copy)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
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
            cursor: (0, 0),
            scroll_offset: 0.0,
            style: crate::theme::from_iced_theme(&iced::Theme::TokyoNightStorm),
            syntax: syntax.to_string(),
            last_blink: Instant::now(),
            cursor_visible: true,
            selection_start: None,
            selection_end: None,
            is_dragging: false,
            cache: canvas::Cache::default(),
            scrollable_id: Id::unique(),
            viewport_scroll: 0.0,
            viewport_height: 600.0, // Default, will be updated
            viewport_width: 800.0,  // Default, will be updated
            history: CommandHistory::new(100),
            is_grouping: false,
            wrap_enabled: true,
            wrap_column: None,
            search_state: search::SearchState::new(),
            translations: Translations::default(),
            search_replace_enabled: true,
            line_numbers_enabled: true,
            has_canvas_focus: false,
            show_cursor: false,
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
        };

        // Perform initial character dimension calculation
        editor.recalculate_char_dimensions(false);

        editor
    }

    /// Sets the font used by the editor
    ///
    /// # Arguments
    ///
    /// * `font` - The iced font to set for the editor
    pub fn set_font(&mut self, font: iced::Font) {
        self.font = font;
        self.recalculate_char_dimensions(false);
    }

    /// Sets the font size and recalculates character dimensions.
    ///
    /// If `auto_adjust_line_height` is true, `line_height` will also be scaled to maintain
    /// the default proportion (Line Height ~ 1.43x).
    ///
    /// # Arguments
    ///
    /// * `size` - The font size in pixels
    /// * `auto_adjust_line_height` - Whether to automatically adjust the line height
    pub fn set_font_size(&mut self, size: f32, auto_adjust_line_height: bool) {
        self.font_size = size;
        self.recalculate_char_dimensions(auto_adjust_line_height);
    }

    /// Recalculates character dimensions based on current font and size.
    fn recalculate_char_dimensions(&mut self, auto_adjust_line_height: bool) {
        self.char_width = self.measure_single_char_width("a");
        // Use '汉' as a standard reference for CJK (Chinese, Japanese, Korean) wide characters
        self.full_char_width = self.measure_single_char_width("汉");

        // Fallback for infinite width measurements
        if self.char_width.is_infinite() {
            self.char_width = self.font_size / 2.0; // Rough estimate for monospace
        }

        if self.full_char_width.is_infinite() {
            self.full_char_width = self.font_size;
        }

        if auto_adjust_line_height {
            let line_height_ratio = LINE_HEIGHT / FONT_SIZE;
            self.line_height = self.font_size * line_height_ratio;
        }

        self.cache.clear();
    }

    /// Measures the width of a single character string using the current font settings.
    fn measure_single_char_width(&self, content: &str) -> f32 {
        let text = Text {
            content,
            font: self.font,
            size: iced::Pixels(self.font_size),
            line_height: iced::advanced::text::LineHeight::default(),
            bounds: iced::Size::new(f32::INFINITY, f32::INFINITY),
            align_x: Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping: iced::advanced::text::Wrapping::default(),
        };
        let p = <iced::Renderer as TextRenderer>::Paragraph::with_text(text);
        p.min_width()
    }

    /// Returns the current font size.
    ///
    /// # Returns
    ///
    /// The font size in pixels
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the width of a standard narrow character in pixels.
    ///
    /// # Returns
    ///
    /// The character width in pixels
    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    /// Returns the width of a wide character (e.g. CJK) in pixels.
    ///
    /// # Returns
    ///
    /// The full character width in pixels
    pub fn full_char_width(&self) -> f32 {
        self.full_char_width
    }

    /// Sets the line height used by the editor
    ///
    /// # Arguments
    ///
    /// * `height` - The line height in pixels
    pub fn set_line_height(&mut self, height: f32) {
        self.line_height = height;
        self.cache.clear();
    }

    /// Returns the current line height.
    ///
    /// # Returns
    ///
    /// The line height in pixels
    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    /// Returns the current text content as a string.
    ///
    /// # Returns
    ///
    /// The complete text content of the editor
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }

    /// Sets the viewport height for the editor.
    ///
    /// This determines the minimum height of the canvas, ensuring proper
    /// background rendering even when content is smaller than the viewport.
    ///
    /// # Arguments
    ///
    /// * `height` - The viewport height in pixels
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_viewport_height(500.0);
    /// ```
    #[must_use]
    pub fn with_viewport_height(mut self, height: f32) -> Self {
        self.viewport_height = height;
        self
    }

    /// Sets the theme style for the editor.
    ///
    /// # Arguments
    ///
    /// * `style` - The style to apply to the editor
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, theme};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_theme(theme::from_iced_theme(&iced::Theme::TokyoNightStorm));
    /// ```
    pub fn set_theme(&mut self, style: Style) {
        self.style = style;
        self.cache.clear(); // Force redraw with new theme
    }

    /// Sets the language for UI translations.
    ///
    /// This changes the language used for all UI text elements in the editor,
    /// including search dialog tooltips, placeholders, and labels.
    ///
    /// # Arguments
    ///
    /// * `language` - The language to use for UI text
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Language};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_language(Language::French);
    /// ```
    pub fn set_language(&mut self, language: crate::i18n::Language) {
        self.translations.set_language(language);
        self.cache.clear(); // Force UI redraw
    }

    /// Returns the current UI language.
    ///
    /// # Returns
    ///
    /// The currently active language for UI text
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Language};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// let current_lang = editor.language();
    /// ```
    pub fn language(&self) -> crate::i18n::Language {
        self.translations.language()
    }

    /// Requests focus for this editor.
    ///
    /// This method programmatically sets the focus to this editor instance,
    /// allowing it to receive keyboard events. Other editors will automatically
    /// lose focus.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor1 = CodeEditor::new("fn main() {}", "rs");
    /// let mut editor2 = CodeEditor::new("fn test() {}", "rs");
    ///
    /// // Give focus to editor2
    /// editor2.request_focus();
    /// ```
    pub fn request_focus(&self) {
        FOCUSED_EDITOR_ID.store(self.editor_id, Ordering::Relaxed);
    }

    /// Checks if this editor currently has focus.
    ///
    /// Returns `true` if this editor will receive keyboard events,
    /// `false` otherwise.
    ///
    /// # Returns
    ///
    /// `true` if focused, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// if editor.is_focused() {
    ///     println!("Editor has focus");
    /// }
    /// ```
    pub fn is_focused(&self) -> bool {
        FOCUSED_EDITOR_ID.load(Ordering::Relaxed) == self.editor_id
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
    /// ```ignore
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("initial content", "lua");
    /// // Later, reset with new content and get the task
    /// let task = editor.reset("new content");
    /// // Return task.map(YourMessage::Editor) from your update function
    /// ```
    pub fn reset(&mut self, content: &str) -> iced::Task<Message> {
        self.buffer = TextBuffer::new(content);
        self.cursor = (0, 0);
        self.scroll_offset = 0.0;
        self.selection_start = None;
        self.selection_end = None;
        self.is_dragging = false;
        self.viewport_scroll = 0.0;
        self.history = CommandHistory::new(100);
        self.is_grouping = false;
        self.last_blink = Instant::now();
        self.cursor_visible = true;
        // Create a new cache to ensure complete redraw (clear() is not sufficient
        // when new content is smaller than previous content)
        self.cache = canvas::Cache::default();

        // Scroll to top to force a redraw
        snap_to(self.scrollable_id.clone(), RelativeOffset::START)
    }

    /// Resets the cursor blink animation.
    pub(crate) fn reset_cursor_blink(&mut self) {
        self.last_blink = Instant::now();
        self.cursor_visible = true;
    }

    /// Refreshes search matches after buffer modification.
    ///
    /// Should be called after any operation that modifies the buffer.
    /// If search is active, recalculates matches and selects the one
    /// closest to the current cursor position.
    pub(crate) fn refresh_search_matches_if_needed(&mut self) {
        if self.search_state.is_open && !self.search_state.query.is_empty() {
            // Recalculate matches with current query
            self.search_state.update_matches(&self.buffer);

            // Select match closest to cursor to maintain context
            self.search_state.select_match_near_cursor(self.cursor);
        }
    }

    /// Returns whether the editor has unsaved changes.
    ///
    /// # Returns
    ///
    /// `true` if there are unsaved modifications, `false` otherwise
    pub fn is_modified(&self) -> bool {
        self.history.is_modified()
    }

    /// Marks the current state as saved.
    ///
    /// Call this after successfully saving the file to reset the modified state.
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Returns whether undo is available.
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns whether redo is available.
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    /// Sets whether line wrapping is enabled.
    ///
    /// When enabled, long lines will wrap at the viewport width or at a
    /// configured column width.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable line wrapping
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_wrap_enabled(false); // Disable wrapping
    /// ```
    pub fn set_wrap_enabled(&mut self, enabled: bool) {
        if self.wrap_enabled != enabled {
            self.wrap_enabled = enabled;
            self.cache.clear(); // Force redraw
        }
    }

    /// Returns whether line wrapping is enabled.
    ///
    /// # Returns
    ///
    /// `true` if line wrapping is enabled, `false` otherwise
    pub fn wrap_enabled(&self) -> bool {
        self.wrap_enabled
    }

    /// Enables or disables the search/replace functionality.
    ///
    /// When disabled, search/replace keyboard shortcuts (Ctrl+F, Ctrl+H, F3)
    /// will be ignored. If the search dialog is currently open, it will be closed.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable search/replace functionality
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_search_replace_enabled(false); // Disable search/replace
    /// ```
    pub fn set_search_replace_enabled(&mut self, enabled: bool) {
        self.search_replace_enabled = enabled;
        if !enabled && self.search_state.is_open {
            self.search_state.close();
        }
    }

    /// Returns whether search/replace functionality is enabled.
    ///
    /// # Returns
    ///
    /// `true` if search/replace is enabled, `false` otherwise
    pub fn search_replace_enabled(&self) -> bool {
        self.search_replace_enabled
    }

    /// Sets the line wrapping with builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable line wrapping
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_wrap_enabled(false);
    /// ```
    #[must_use]
    pub fn with_wrap_enabled(mut self, enabled: bool) -> Self {
        self.wrap_enabled = enabled;
        self
    }

    /// Sets the wrap column (fixed width wrapping).
    ///
    /// When set to `Some(n)`, lines will wrap at column `n`.
    /// When set to `None`, lines will wrap at the viewport width.
    ///
    /// # Arguments
    ///
    /// * `column` - The column to wrap at, or None for viewport-based wrapping
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_wrap_column(Some(80)); // Wrap at 80 characters
    /// ```
    #[must_use]
    pub fn with_wrap_column(mut self, column: Option<usize>) -> Self {
        self.wrap_column = column;
        self
    }

    /// Sets whether line numbers are displayed.
    ///
    /// When disabled, the gutter is completely removed (0px width),
    /// providing more space for code display.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to display line numbers
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_line_numbers_enabled(false); // Hide line numbers
    /// ```
    pub fn set_line_numbers_enabled(&mut self, enabled: bool) {
        if self.line_numbers_enabled != enabled {
            self.line_numbers_enabled = enabled;
            self.cache.clear(); // Force redraw
        }
    }

    /// Returns whether line numbers are displayed.
    ///
    /// # Returns
    ///
    /// `true` if line numbers are displayed, `false` otherwise
    pub fn line_numbers_enabled(&self) -> bool {
        self.line_numbers_enabled
    }

    /// Sets the line numbers display with builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to display line numbers
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_line_numbers_enabled(false);
    /// ```
    #[must_use]
    pub fn with_line_numbers_enabled(mut self, enabled: bool) -> Self {
        self.line_numbers_enabled = enabled;
        self
    }

    /// Returns the current gutter width based on whether line numbers are enabled.
    ///
    /// # Returns
    ///
    /// `GUTTER_WIDTH` if line numbers are enabled, `0.0` otherwise
    pub(crate) fn gutter_width(&self) -> f32 {
        if self.line_numbers_enabled { GUTTER_WIDTH } else { 0.0 }
    }

    /// Removes canvas focus from this editor.
    ///
    /// This method programmatically removes focus from the canvas, preventing
    /// it from receiving keyboard events. The cursor will be hidden, but the
    /// selection will remain visible.
    ///
    /// Call this when focus should move to another widget (e.g., text input).
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.lose_focus();
    /// ```
    pub fn lose_focus(&mut self) {
        self.has_canvas_focus = false;
        self.show_cursor = false;
        self.ime_preedit = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_floats() {
        // Equal cases
        assert_eq!(
            compare_floats(1.0, 1.0),
            CmpOrdering::Equal,
            "Exact equality"
        );
        assert_eq!(
            compare_floats(1.0, 1.0 + 0.0001),
            CmpOrdering::Equal,
            "Within epsilon (positive)"
        );
        assert_eq!(
            compare_floats(1.0, 1.0 - 0.0001),
            CmpOrdering::Equal,
            "Within epsilon (negative)"
        );

        // Greater cases
        assert_eq!(
            compare_floats(1.0 + 0.002, 1.0),
            CmpOrdering::Greater,
            "Definitely greater"
        );
        assert_eq!(
            compare_floats(1.0011, 1.0),
            CmpOrdering::Greater,
            "Just above epsilon"
        );

        // Less cases
        assert_eq!(
            compare_floats(1.0, 1.0 + 0.002),
            CmpOrdering::Less,
            "Definitely less"
        );
        assert_eq!(
            compare_floats(1.0, 1.0011),
            CmpOrdering::Less,
            "Just below negative epsilon"
        );
    }

    #[test]
    fn test_measure_text_width_ascii() {
        // "abc" (3 chars) -> 3 * CHAR_WIDTH
        let text = "abc";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = CHAR_WIDTH * 3.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for ASCII"
        );
    }

    #[test]
    fn test_measure_text_width_cjk() {
        // "你好" (2 chars) -> 2 * FONT_SIZE
        // Chinese characters are typically full-width.
        // width = 2 * FONT_SIZE
        let text = "你好";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = FONT_SIZE * 2.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for CJK"
        );
    }

    #[test]
    fn test_measure_text_width_mixed() {
        // "Hi" (2 chars) -> 2 * CHAR_WIDTH
        // "你好" (2 chars) -> 2 * FONT_SIZE
        let text = "Hi你好";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = CHAR_WIDTH * 2.0 + FONT_SIZE * 2.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for mixed content"
        );
    }

    #[test]
    fn test_measure_text_width_control_chars() {
        // "\t\n" (2 chars)
        // width = 0.0 (control chars have 0 width in this implementation)
        let text = "\t\n";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = 0.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for control chars"
        );
    }

    #[test]
    fn test_measure_text_width_empty() {
        let text = "";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        assert!(
            (width - 0.0).abs() < f32::EPSILON,
            "Width should be 0 for empty string"
        );
    }

    #[test]
    fn test_measure_text_width_emoji() {
        // "👋" (1 char, width > 1) -> FONT_SIZE
        let text = "👋";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = FONT_SIZE;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for emoji"
        );
    }

    #[test]
    fn test_measure_text_width_korean() {
        // "안녕하세요" (5 chars)
        // Korean characters are typically full-width.
        // width = 5 * FONT_SIZE
        let text = "안녕하세요";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = FONT_SIZE * 5.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for Korean"
        );
    }

    #[test]
    fn test_measure_text_width_japanese() {
        // "こんにちは" (Hiragana, 5 chars) -> 5 * FONT_SIZE
        // "カタカナ" (Katakana, 4 chars) -> 4 * FONT_SIZE
        // "漢字" (Kanji, 2 chars) -> 2 * FONT_SIZE

        let text_hiragana = "こんにちは";
        let width_hiragana =
            measure_text_width(text_hiragana, FONT_SIZE, CHAR_WIDTH);
        let expected_hiragana = FONT_SIZE * 5.0;
        assert_eq!(
            compare_floats(width_hiragana, expected_hiragana),
            CmpOrdering::Equal,
            "Width mismatch for Hiragana"
        );

        let text_katakana = "カタカナ";
        let width_katakana =
            measure_text_width(text_katakana, FONT_SIZE, CHAR_WIDTH);
        let expected_katakana = FONT_SIZE * 4.0;
        assert_eq!(
            compare_floats(width_katakana, expected_katakana),
            CmpOrdering::Equal,
            "Width mismatch for Katakana"
        );

        let text_kanji = "漢字";
        let width_kanji = measure_text_width(text_kanji, FONT_SIZE, CHAR_WIDTH);
        let expected_kanji = FONT_SIZE * 2.0;
        assert_eq!(
            compare_floats(width_kanji, expected_kanji),
            CmpOrdering::Equal,
            "Width mismatch for Kanji"
        );
    }

    #[test]
    fn test_set_font_size() {
        let mut editor = CodeEditor::new("", "rs");

        // Initial state (defaults)
        assert!((editor.font_size() - 14.0).abs() < f32::EPSILON);
        assert!((editor.line_height() - 20.0).abs() < f32::EPSILON);

        // Test auto adjust = true
        editor.set_font_size(28.0, true);
        assert!((editor.font_size() - 28.0).abs() < f32::EPSILON);
        // Line height should double: 20.0 * (28.0/14.0) = 40.0
        assert_eq!(
            compare_floats(editor.line_height(), 40.0),
            CmpOrdering::Equal
        );

        // Test auto adjust = false
        // First set line height to something custom
        editor.set_line_height(50.0);
        // Change font size but keep line height
        editor.set_font_size(14.0, false);
        assert!((editor.font_size() - 14.0).abs() < f32::EPSILON);
        // Line height should stay 50.0
        assert_eq!(
            compare_floats(editor.line_height(), 50.0),
            CmpOrdering::Equal
        );
        // Char width should have scaled back to roughly default (but depends on measurement)
        // We check if it is close to the expected value, but since measurement can vary,
        // we just ensure it is positive and close to what we expect (around 8.4)
        assert!(editor.char_width > 0.0);
        assert!((editor.char_width - CHAR_WIDTH).abs() < 0.5);
    }

    #[test]
    fn test_measure_single_char_width() {
        let editor = CodeEditor::new("", "rs");

        // Measure 'a'
        let width_a = editor.measure_single_char_width("a");
        assert!(width_a > 0.0, "Width of 'a' should be positive");

        // Measure Chinese char
        let width_cjk = editor.measure_single_char_width("汉");
        assert!(width_cjk > 0.0, "Width of '汉' should be positive");

        assert!(
            width_cjk > width_a,
            "Width of '汉' should be greater than 'a'"
        );

        // Check that width_cjk is roughly double of width_a (common in terminal fonts)
        // but we just check it is significantly larger
        assert!(width_cjk >= width_a * 1.5);
    }

    #[test]
    fn test_set_line_height() {
        let mut editor = CodeEditor::new("", "rs");

        // Initial state
        assert!((editor.line_height() - LINE_HEIGHT).abs() < f32::EPSILON);

        // Set custom line height
        editor.set_line_height(35.0);
        assert!((editor.line_height() - 35.0).abs() < f32::EPSILON);

        // Font size should remain unchanged
        assert!((editor.font_size() - FONT_SIZE).abs() < f32::EPSILON);
    }
}
