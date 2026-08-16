//! Command pattern implementation for undo/redo functionality.
//!
//! This module provides a trait-based command system that allows all text
//! modifications to be recorded and reversed, enabling robust undo/redo support.
//!
//! Concrete commands are grouped by family into submodules: [`edit`] for
//! single-buffer editing, [`composite`] for grouping/replace commands,
//! [`lines`] for line-level moves/duplication, and [`comment`] for line-
//! comment toggling.

use crate::text_buffer::TextBuffer;

mod comment;
mod composite;
mod edit;
mod lines;

pub use comment::ToggleCommentCommand;
pub(crate) use comment::line_comment_token;
pub use composite::{CompositeCommand, ReplaceTextCommand};
pub use edit::{
    DeleteCharCommand, DeleteForwardCommand, DeleteRangeCommand,
    InsertCharCommand, InsertNewlineCommand, InsertTextCommand,
};
pub use lines::{DuplicateLinesCommand, MoveLinesCommand};

/// Trait for reversible editor commands.
///
/// All text modifications should implement this trait to support undo/redo.
/// Commands must be both executable and reversible.
pub trait Command: Send + std::fmt::Debug {
    /// Executes the command, modifying the buffer and cursor.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to modify
    /// * `cursor` - The cursor position (will be updated)
    fn execute(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize));

    /// Reverses the command, restoring previous state.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to modify
    /// * `cursor` - The cursor position (will be restored)
    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize));
}
