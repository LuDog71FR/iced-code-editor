//! Canvas-based text editor widget for maximum performance.
//!
//! This module provides a custom Canvas widget that handles all text rendering
//! and input directly, bypassing Iced's higher-level widgets for optimal speed.

use iced::widget::Id;
use iced::widget::canvas;
use std::time::Instant;

use crate::text_buffer::TextBuffer;
use crate::theme::Style;
pub use history::CommandHistory;

// Re-export submodules
mod canvas_impl;
mod clipboard;
pub mod command;
mod cursor;
pub mod history;
mod selection;
mod update;
mod view;

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
    /// Command history for undo/redo
    pub(crate) history: CommandHistory,
    /// Whether we're currently grouping commands (for smart undo)
    pub(crate) is_grouping: bool,
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
            style: crate::theme::dark(&iced::Theme::Dark),
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
            history: CommandHistory::new(100),
            is_grouping: false,
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
    /// editor.set_theme(theme::light(&iced::Theme::Light));
    /// ```
    pub fn set_theme(&mut self, style: Style) {
        self.style = style;
        self.cache.clear(); // Force redraw with new theme
    }

    /// Resets the cursor blink animation.
    pub(crate) fn reset_cursor_blink(&mut self) {
        self.last_blink = Instant::now();
        self.cursor_visible = true;
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
}
