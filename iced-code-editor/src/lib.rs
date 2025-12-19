//! A high-performance code editor widget for the Iced GUI framework.
//!
//! This crate provides a canvas-based code editor with:
//! - Syntax highlighting for multiple programming languages (Python, Lua, Rust, JavaScript, etc.)
//! - Line numbers display with styled gutter
//! - Text selection (mouse drag and keyboard)
//! - Clipboard operations (copy, paste)
//! - Scrollbars with custom styling
//! - Focus management for multiple editors
//! - Dark theme support with customizable colors
//!
//! # Example
//!
//! ```ignore
//! use iced_code_editor::{CanvasEditor, CanvasEditorMessage};
//!
//! // Create a Python editor
//! let editor = CanvasEditor::new("print('Hello, World!')", "py");
//!
//! // Create a Rust editor
//! let rust_editor = CanvasEditor::new("fn main() {}", "rs");
//! ```
//!
//! # Supported Languages
//!
//! The editor supports syntax highlighting for many languages through the `syntect` crate:
//! - Python (`"py"` or `"python"`)
//! - Lua (`"lua"`)
//! - Rust (`"rs"` or `"rust"`)
//! - JavaScript (`"js"` or `"javascript"`)
//! - And many more...
//!
//! For a complete list, refer to the `syntect` crate documentation.

mod canvas_editor;
mod state;
mod text_buffer;

pub use canvas_editor::{CanvasEditor, CanvasEditorMessage};
pub use state::EditorTheme;
