//! The editing model: cursors, selection, clipboard, undo/redo history, and
//! the command pattern that backs them.

mod clipboard;
pub mod command;
mod cursor;
pub(crate) mod cursor_set;
pub mod history;
mod selection;
