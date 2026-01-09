//! Canvas-based text editor widget for maximum performance.
//!
//! This module provides a custom Canvas widget that handles all text rendering
//! and input directly, bypassing Iced's higher-level widgets for optimal speed.

use iced::widget::operation::{RelativeOffset, snap_to};
use iced::widget::{Id, canvas};
use std::time::Instant;

use crate::i18n::Translations;
use crate::text_buffer::TextBuffer;
use crate::theme::Style;
pub use history::CommandHistory;

// Re-export submodules
mod canvas_impl;
mod clipboard;
pub mod command;
mod cursor;
pub mod history;
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
pub(crate) const GUTTER_WIDTH: f32 = 60.0;
pub(crate) const CURSOR_BLINK_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(530);

/// Canvas-based high-performance text editor.
pub struct CodeEditor {
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
        Self {
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
        }
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
    /// The current language setting
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Language};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert_eq!(editor.language(), Language::English);
    /// ```
    #[must_use]
    pub const fn language(&self) -> crate::i18n::Language {
        self.translations.language()
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
}
