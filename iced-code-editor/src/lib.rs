//! A code editor widget for the Iced GUI framework.
//!
//! This crate provides a full-featured code editor component with:
//! - Syntax highlighting for multiple programming languages (Python, Lua, Rust, JavaScript, etc.)
//! - Line numbers display with styled gutter
//! - Smart cursor tracking and auto-scrolling
//! - Custom key bindings (e.g., Tab inserts 4 spaces)
//! - Dark theme support with customizable colors
//!
//! # Example
//!
//! ```ignore
//! use iced_code_editor::CodeEditorComponent;
//!
//! // Create a Python editor
//! let editor = CodeEditorComponent::new_with_language("print('Hello, World!')", "py");
//!
//! // Create a Rust editor
//! let rust_editor = CodeEditorComponent::new_with_language("fn main() {}", "rs");
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
//! For a complete list, refer to the `syntect` crate documentation or
//! Iced's highlighter module.

mod canvas_editor;
mod component;
mod custom_editor;
mod state;
mod text_buffer;

pub use canvas_editor::{CanvasEditor, CanvasEditorMessage};
pub use component::{CodeEditorComponent, Event};
pub use custom_editor::{CustomEditor, CustomEditorEvent};
pub use state::EditorTheme;
