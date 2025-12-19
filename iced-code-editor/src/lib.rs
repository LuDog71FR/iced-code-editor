//! A high-performance code editor widget for Iced.
//!
//! This crate provides a canvas-based code editor with syntax highlighting,
//! line numbers, and text selection capabilities for the Iced GUI framework.
//!
//! # Features
//!
//! - **Syntax highlighting** for multiple programming languages
//! - **Line numbers** with styled gutter
//! - **Text selection** via mouse drag and keyboard
//! - **Clipboard operations** (copy, paste)
//! - **Custom scrollbars** with themed styling
//! - **Focus management** for multiple editors
//! - **Dark & light themes** support with customizable colors
//! - **Undo/Redo** with command history
//!
//! # Example
//!
//! ```no_run
//! use iced::widget::Column;
//! use iced::{Element, Task};
//! use iced_code_editor::{CodeEditor, Message};
//!
//! struct App {
//!     editor: CodeEditor,
//! }
//!
//! #[derive(Debug, Clone)]
//! enum AppMessage {
//!     EditorAction(Message),
//! }
//!
//! impl App {
//!     fn new() -> (Self, Task<AppMessage>) {
//!         (
//!             Self {
//!                 editor: CodeEditor::new("fn main() {}", "rs"),
//!             },
//!             Task::none(),
//!         )
//!     }
//!
//!     fn update(&mut self, message: AppMessage) -> Task<AppMessage> {
//!         match message {
//!             AppMessage::EditorAction(action) => {
//!                 self.editor
//!                     .update(&action)
//!                     .map(AppMessage::EditorAction)
//!             }
//!         }
//!     }
//!
//!     fn view(&self) -> Element<AppMessage> {
//!         Column::new()
//!             .push(self.editor.view().map(AppMessage::EditorAction))
//!             .into()
//!     }
//! }
//! ```
//!
//! # Themes
//!
//! The editor supports both dark and light themes:
//!
//! ```no_run
//! use iced_code_editor::{CodeEditor, theme};
//!
//! // Create an editor with dark theme (default)
//! let mut editor = CodeEditor::new("fn main() {}", "rs");
//!
//! // Switch to light theme
//! editor.set_theme(theme::light(&iced::Theme::Light));
//! ```
//!
//! # Keyboard Shortcuts
//!
//! The editor supports a comprehensive set of keyboard shortcuts:
//!
//! ## Navigation
//!
//! | Shortcut | Action |
//! |----------|--------|
//! | **Arrow Keys** (Up, Down, Left, Right) | Move cursor |
//! | **Shift + Arrows** | Move cursor with selection |
//! | **Home** / **End** | Jump to start/end of line |
//! | **Shift + Home** / **Shift + End** | Select to start/end of line |
//! | **Ctrl + Home** / **Ctrl + End** | Jump to start/end of document |
//! | **Page Up** / **Page Down** | Scroll one page up/down |
//!
//! ## Editing
//!
//! | Shortcut | Action |
//! |----------|--------|
//! | **Backspace** | Delete character before cursor |
//! | **Delete** | Delete character after cursor |
//! | **Shift + Delete** | Delete selected text |
//! | **Enter** | Insert new line |
//!
//! ## Clipboard
//!
//! | Shortcut | Action |
//! |----------|--------|
//! | **Ctrl + C** or **Ctrl + Insert** | Copy selected text |
//! | **Ctrl + V** or **Shift + Insert** | Paste from clipboard |
//!
//! # Supported Languages
//!
//! The editor supports syntax highlighting through the `syntect` crate:
//! - Python (`"py"` or `"python"`)
//! - Lua (`"lua"`)
//! - Rust (`"rs"` or `"rust"`)
//! - JavaScript (`"js"` or `"javascript"`)
//! - And many more...
//!
//! For a complete list, refer to the `syntect` crate documentation.
//!
//! # Command History Management
//!
//! The [`CommandHistory`] type provides fine-grained control over undo/redo operations.
//! While the editor handles history automatically, you can access it directly for
//! advanced use cases:
//!
//! ## Monitoring History State
//!
//! ```no_run
//! use iced_code_editor::CommandHistory;
//!
//! let history = CommandHistory::new(100);
//!
//! // Check how many operations are available
//! println!("Undo operations: {}", history.undo_count());
//! println!("Redo operations: {}", history.redo_count());
//!
//! // Check if operations are possible
//! if history.can_undo() {
//!     println!("Can undo!");
//! }
//! ```
//!
//! ## Adjusting History Size
//!
//! You can dynamically adjust the maximum number of operations kept in history:
//!
//! ```no_run
//! use iced_code_editor::CommandHistory;
//!
//! let history = CommandHistory::new(100);
//!
//! // Get current maximum
//! assert_eq!(history.max_size(), 100);
//!
//! // Increase limit for memory-rich environments
//! history.set_max_size(500);
//!
//! // Or decrease for constrained environments
//! history.set_max_size(50);
//! ```
//!
//! ## Clearing History
//!
//! You can reset the entire history when needed:
//!
//! ```no_run
//! use iced_code_editor::CommandHistory;
//!
//! let history = CommandHistory::new(100);
//!
//! // Clear all undo/redo operations
//! history.clear();
//!
//! assert_eq!(history.undo_count(), 0);
//! assert_eq!(history.redo_count(), 0);
//! ```
//!
//! ## Save Point Tracking
//!
//! Track whether the document has been modified since the last save:
//!
//! ```no_run
//! use iced_code_editor::CommandHistory;
//!
//! let history = CommandHistory::new(100);
//!
//! // After loading or saving a file
//! history.mark_saved();
//!
//! // Check if there are unsaved changes
//! if history.is_modified() {
//!     println!("Document has unsaved changes!");
//! }
//! ```

mod canvas_editor;
mod text_buffer;

pub mod theme;

pub use canvas_editor::{ArrowDirection, CodeEditor, CommandHistory, Message};
pub use theme::{Catalog, Style, StyleFn, dark, light};
