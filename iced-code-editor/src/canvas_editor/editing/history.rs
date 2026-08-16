//! Command history management for undo/redo functionality.
//!
//! This module provides thread-safe command history tracking with configurable
//! size limits and save point tracking for modified state detection.
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```
//! use iced_code_editor::CommandHistory;
//!
//! // Create a history with a limit of 100 operations
//! let history = CommandHistory::new(100);
//!
//! // Check state
//! assert_eq!(history.undo_count(), 0);
//! assert_eq!(history.redo_count(), 0);
//! assert!(!history.can_undo());
//! ```
//!
//! ## Dynamic Configuration
//!
//! ```
//! use iced_code_editor::CommandHistory;
//!
//! let history = CommandHistory::new(100);
//!
//! // Adjust history size based on available memory
//! history.set_max_size(500);
//! assert_eq!(history.max_size(), 500);
//!
//! // Clear all history when starting a new document
//! history.clear();
//! ```
//!
//! ## Save Point Tracking
//!
//! ```
//! use iced_code_editor::CommandHistory;
//!
//! let history = CommandHistory::new(100);
//!
//! // Mark the current state as saved
//! history.mark_saved();
//! assert!(!history.is_modified());
//!
//! // After user makes changes...
//! // history.push(some_command);
//! // assert!(history.is_modified());
//! ```

// Allow unwrap on Mutex since this is safe in the single-threaded GUI context
// The mutex is only used for interior mutability, not actual multi-threading
#![allow(clippy::unwrap_used)]
// The Mutex cannot be poisoned in our single-threaded context, so panics documented
// below would never actually occur in practice
#![allow(clippy::missing_panics_doc)]

use super::command::{Command, CompositeCommand};
use crate::buffer::TextBuffer;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Manages command history for undo/redo operations.
///
/// The history maintains two stacks:
/// - Undo stack: Commands that can be undone
/// - Redo stack: Commands that can be redone (cleared when new commands are added)
///
/// Thread-safe using Arc<Mutex<>> for interior mutability.
#[derive(Debug, Clone)]
pub struct CommandHistory {
    inner: Arc<Mutex<HistoryInner>>,
}

#[derive(Debug)]
struct HistoryInner {
    /// Stack of commands that can be undone
    undo_stack: VecDeque<Box<dyn Command>>,
    /// Stack of commands that can be redone
    redo_stack: Vec<Box<dyn Command>>,
    /// Maximum number of commands to keep in history
    max_size: usize,
    /// Index in undo_stack where document was last saved (None if never saved)
    save_point: Option<usize>,
    /// Current composite command being built (for grouping)
    current_group: Option<CompositeCommand>,
}

impl HistoryInner {
    /// Trims `undo_stack` down to `max_size`, discarding the oldest commands
    /// first and shifting `save_point` to stay aligned with what remains.
    ///
    /// If a discarded command was the one marked as the save point, the save
    /// point is cleared instead, since that saved state is no longer
    /// reachable via undo.
    fn enforce_size_limit(&mut self) {
        while self.undo_stack.len() > self.max_size {
            self.undo_stack.pop_front();
            if let Some(ref mut sp) = self.save_point {
                if *sp > 0 {
                    *sp -= 1;
                } else {
                    self.save_point = None;
                }
            }
        }
    }
}

impl CommandHistory {
    /// Creates a new command history with the specified size limit.
    ///
    /// # Arguments
    ///
    /// * `max_size` - Maximum number of commands to keep in history
    ///
    /// # Returns
    ///
    /// A new `CommandHistory` instance
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// ```
    pub fn new(max_size: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HistoryInner {
                undo_stack: VecDeque::with_capacity(max_size.min(100)),
                redo_stack: Vec::with_capacity(max_size.min(100)),
                max_size,
                save_point: None,
                current_group: None,
            })),
        }
    }

    /// Adds a command to the history.
    ///
    /// This clears the redo stack and adds the command to the undo stack.
    /// If currently grouping commands, adds to the current group instead.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to add
    pub fn push(&self, command: Box<dyn Command>) {
        let mut inner = self.inner.lock().unwrap();

        // If we're building a composite, add to it
        if let Some(ref mut group) = inner.current_group {
            group.add(command);
            return;
        }

        // The save point may sit ahead of the current position (the user
        // undid past it). Pushing a new command here clears the redo stack,
        // permanently discarding the path back to that saved state, so the
        // save point can never be reached again and must be invalidated.
        // Otherwise, if the new undo stack happens to reach the same length
        // later, `is_modified()` would wrongly report "not modified" even
        // though the actual command sequence has diverged.
        if inner.save_point.is_some_and(|sp| sp > inner.undo_stack.len()) {
            inner.save_point = None;
        }

        // Clear redo stack when new command is added
        inner.redo_stack.clear();

        // Add to undo stack
        inner.undo_stack.push_back(command);

        inner.enforce_size_limit();
    }

    /// Undoes the last command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to modify
    /// * `cursor` - The cursor position to update
    ///
    /// # Returns
    ///
    /// `true` if a command was undone, `false` if nothing to undo
    pub fn undo(
        &self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) -> bool {
        let mut inner = self.inner.lock().unwrap();

        // End any current grouping
        if inner.current_group.is_some() {
            Self::end_group_internal(&mut inner);
        }

        if let Some(mut command) = inner.undo_stack.pop_back() {
            command.undo(buffer, cursor);
            inner.redo_stack.push(command);
            true
        } else {
            false
        }
    }

    /// Redoes the last undone command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to modify
    /// * `cursor` - The cursor position to update
    ///
    /// # Returns
    ///
    /// `true` if a command was redone, `false` if nothing to redo
    pub fn redo(
        &self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) -> bool {
        let mut inner = self.inner.lock().unwrap();

        if let Some(mut command) = inner.redo_stack.pop() {
            command.execute(buffer, cursor);
            inner.undo_stack.push_back(command);
            true
        } else {
            false
        }
    }

    /// Returns whether there are commands that can be undone.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.undo_stack.is_empty() || inner.current_group.is_some()
    }

    /// Returns whether there are commands that can be redone.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.redo_stack.is_empty()
    }

    /// Marks the current position as the save point.
    ///
    /// This is used to track whether the document has been modified since
    /// the last save. Call this after successfully saving the file.
    pub fn mark_saved(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.save_point = Some(inner.undo_stack.len());
    }

    /// Returns whether the document has been modified since the last save.
    ///
    /// # Returns
    ///
    /// `true` if there are unsaved changes, `false` otherwise
    #[must_use]
    pub fn is_modified(&self) -> bool {
        let inner = self.inner.lock().unwrap();

        // If we're currently in a group, we're modified
        if inner.current_group.is_some() {
            return true;
        }

        match inner.save_point {
            None => !inner.undo_stack.is_empty(),
            Some(sp) => sp != inner.undo_stack.len(),
        }
    }

    /// Clears all history.
    ///
    /// This removes all undo/redo commands and resets the save point.
    /// Useful when starting a new document or resetting the editor state.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// // ... perform some operations ...
    ///
    /// // Clear everything when opening a new document
    /// history.clear();
    /// assert_eq!(history.undo_count(), 0);
    /// assert_eq!(history.redo_count(), 0);
    /// assert!(!history.is_modified());
    /// ```
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.undo_stack.clear();
        inner.redo_stack.clear();
        inner.save_point = None;
        inner.current_group = None;
    }

    /// Begins grouping subsequent commands into a composite.
    ///
    /// All commands added via `push()` will be grouped together until
    /// `end_group()` is called. This is useful for grouping consecutive
    /// typing operations.
    pub fn begin_group(&self) {
        let mut inner = self.inner.lock().unwrap();
        if inner.current_group.is_none() {
            inner.current_group = Some(CompositeCommand::new());
        }
    }

    /// Ends the current command grouping.
    ///
    /// The grouped commands are added to the history as a single composite
    /// command. If no commands were grouped, nothing is added.
    pub fn end_group(&self) {
        let mut inner = self.inner.lock().unwrap();
        Self::end_group_internal(&mut inner);
    }

    /// Internal helper to end grouping (used when lock is already held).
    fn end_group_internal(inner: &mut HistoryInner) {
        if let Some(group) = inner.current_group.take()
            && !group.is_empty()
        {
            // See the matching comment in `push`: a save point ahead of the
            // current position can never be reached again once the redo
            // stack backing it is cleared here.
            if inner.save_point.is_some_and(|sp| sp > inner.undo_stack.len()) {
                inner.save_point = None;
            }

            // Clear redo stack
            inner.redo_stack.clear();

            // Add composite to undo stack
            inner.undo_stack.push_back(Box::new(group));

            inner.enforce_size_limit();
        }
    }

    /// Returns the maximum history size.
    ///
    /// # Returns
    ///
    /// The maximum number of commands that can be stored in history.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// assert_eq!(history.max_size(), 100);
    /// ```
    #[must_use]
    pub fn max_size(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.max_size
    }

    /// Sets the maximum history size.
    ///
    /// If the current history exceeds the new size, older commands are removed.
    /// This is useful for adjusting memory usage based on system resources.
    ///
    /// # Arguments
    ///
    /// * `max_size` - New maximum size (number of commands to keep)
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    ///
    /// // Increase limit for memory-rich environments
    /// history.set_max_size(500);
    /// assert_eq!(history.max_size(), 500);
    ///
    /// // Decrease limit for constrained environments
    /// history.set_max_size(50);
    /// assert_eq!(history.max_size(), 50);
    /// ```
    pub fn set_max_size(&self, max_size: usize) {
        let mut inner = self.inner.lock().unwrap();
        inner.max_size = max_size;
        inner.enforce_size_limit();
    }

    /// Returns the current number of undo operations available.
    ///
    /// This can be useful for displaying history statistics or managing
    /// UI state (e.g., enabling/disabling undo buttons).
    ///
    /// # Returns
    ///
    /// The number of commands that can be undone.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// assert_eq!(history.undo_count(), 0);
    ///
    /// // After adding commands...
    /// // assert!(history.undo_count() > 0);
    /// ```
    #[must_use]
    pub fn undo_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.undo_stack.len()
    }

    /// Returns the current number of redo operations available.
    ///
    /// This can be useful for displaying history statistics or managing
    /// UI state (e.g., enabling/disabling redo buttons).
    ///
    /// # Returns
    ///
    /// The number of commands that can be redone.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// assert_eq!(history.redo_count(), 0);
    ///
    /// // After undoing some commands...
    /// // assert!(history.redo_count() > 0);
    /// ```
    #[must_use]
    pub fn redo_count(&self) -> usize {
        let inner = self.inner.lock().unwrap();
        inner.redo_stack.len()
    }
}

// Implement Default for convenient usage
impl Default for CommandHistory {
    fn default() -> Self {
        Self::new(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::editing::command::InsertCharCommand;

    #[test]
    fn test_new_history() {
        let history = CommandHistory::new(50);
        assert_eq!(history.max_size(), 50);
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn test_push_and_undo() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        let mut cmd = InsertCharCommand::new(0, 5, '!', cursor);
        cmd.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd));

        assert!(history.can_undo());
        assert_eq!(buffer.line(0), "hello!");

        history.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_redo() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        let mut cmd = InsertCharCommand::new(0, 5, '!', cursor);
        cmd.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd));

        history.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");

        assert!(history.can_redo());
        history.redo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello!");
        assert_eq!(cursor, (0, 6));
    }

    #[test]
    fn test_save_point() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        assert!(!history.is_modified()); // New document is not modified

        let mut cmd = InsertCharCommand::new(0, 5, '!', cursor);
        cmd.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd));

        assert!(history.is_modified()); // Now modified

        history.mark_saved();
        assert!(!history.is_modified()); // Saved

        let mut cmd2 = InsertCharCommand::new(0, 6, '?', cursor);
        cmd2.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd2));

        assert!(history.is_modified()); // Modified again
    }

    #[test]
    fn test_save_point_invalidated_after_undo_then_diverging_edit() {
        // Regression test: save -> undo -> push a new (different) command
        // must never be reported as "not modified", even if the undo stack
        // happens to return to the same length as the save point.
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        // Type 'A' and mark the document as saved (undo depth 1).
        let mut cmd_a = InsertCharCommand::new(0, 5, 'A', cursor);
        cmd_a.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd_a));
        history.mark_saved();
        assert!(!history.is_modified());

        // Undo it (undo depth 0), then type a *different* character 'B'.
        history.undo(&mut buffer, &mut cursor);
        assert!(history.is_modified());
        let mut cmd_b = InsertCharCommand::new(0, 5, 'B', cursor);
        cmd_b.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd_b));

        // The undo depth (1) matches the save point again, but the buffer
        // content ("helloB") differs from what was actually saved
        // ("helloA"): the document must still be reported as modified.
        assert_eq!(buffer.line(0), "helloB");
        assert!(history.is_modified());
    }

    #[test]
    fn test_save_point_invalidated_after_undo_then_group_diverges() {
        // Same regression as above, but through `begin_group`/`end_group`
        // (composite commands), which has its own save-point handling.
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        history.begin_group();
        let mut cmd_a = InsertCharCommand::new(0, 5, 'A', cursor);
        cmd_a.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd_a));
        history.end_group();
        history.mark_saved();
        assert!(!history.is_modified());

        history.undo(&mut buffer, &mut cursor);
        assert!(history.is_modified());

        history.begin_group();
        let mut cmd_b = InsertCharCommand::new(0, 5, 'B', cursor);
        cmd_b.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd_b));
        history.end_group();

        assert_eq!(buffer.line(0), "helloB");
        assert!(history.is_modified());
    }

    #[test]
    fn test_clear() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        let mut cmd = InsertCharCommand::new(0, 5, '!', cursor);
        cmd.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd));

        assert!(history.can_undo());
        history.clear();
        assert!(!history.can_undo());
        assert!(!history.is_modified());
    }

    #[test]
    fn test_size_limit() {
        let mut buffer = TextBuffer::new("a");
        let mut cursor = (0, 1);
        let history = CommandHistory::new(3);

        // Add 5 commands (exceeds limit of 3)
        for i in 0..5 {
            let mut cmd = InsertCharCommand::new(0, 1 + i, 'x', cursor);
            cmd.execute(&mut buffer, &mut cursor);
            cursor.1 += 1;
            history.push(Box::new(cmd));
        }

        // Should only have 3 in history
        assert_eq!(history.undo_count(), 3);
    }

    #[test]
    fn test_grouping() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        history.begin_group();

        // Add multiple characters
        for ch in "!!!".chars() {
            let mut cmd = InsertCharCommand::new(0, cursor.1, ch, cursor);
            cmd.execute(&mut buffer, &mut cursor);
            // Don't manually increment cursor - execute() does it
            history.push(Box::new(cmd));
        }

        history.end_group();

        assert_eq!(buffer.line(0), "hello!!!");
        assert_eq!(history.undo_count(), 1); // All grouped into one

        // Single undo should remove all three characters
        history.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "hello");
        assert_eq!(cursor, (0, 5));
    }

    #[test]
    fn test_enforce_size_limit_adjusts_save_point() {
        let mut buffer = TextBuffer::new("a");
        let mut cursor = (0, 1);
        let history = CommandHistory::new(3);

        // Fill to the limit and mark this state as saved.
        for i in 0..3 {
            let mut cmd = InsertCharCommand::new(0, 1 + i, 'x', cursor);
            cmd.execute(&mut buffer, &mut cursor);
            cursor.1 += 1;
            history.push(Box::new(cmd));
        }
        history.mark_saved();
        assert!(!history.is_modified());

        // Pushing one more command exceeds max_size (3), trimming the
        // oldest command and shifting the save point down by one to stay
        // aligned with the commands that remain.
        let mut cmd = InsertCharCommand::new(0, cursor.1, 'x', cursor);
        cmd.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd));
        assert_eq!(history.undo_count(), 3);
        assert!(history.is_modified()); // the just-pushed command is unsaved

        // Undoing the new command should land exactly back on the saved
        // state, proving the save point still points at the correct
        // (shifted) index after trimming.
        history.undo(&mut buffer, &mut cursor);
        assert!(!history.is_modified());
    }

    #[test]
    fn test_enforce_size_limit_clears_save_point_when_trimmed_away() {
        let mut buffer = TextBuffer::new("a");
        let mut cursor = (0, 1);
        let history = CommandHistory::new(3);

        // Mark saved at the very first command (save_point = 1), then push
        // enough commands that the first one gets trimmed away entirely.
        let mut cmd = InsertCharCommand::new(0, 1, 'x', cursor);
        cmd.execute(&mut buffer, &mut cursor);
        cursor.1 += 1;
        history.push(Box::new(cmd));
        history.mark_saved();

        for i in 0..3 {
            let mut cmd = InsertCharCommand::new(0, cursor.1 + i, 'x', cursor);
            cmd.execute(&mut buffer, &mut cursor);
            cursor.1 += 1;
            history.push(Box::new(cmd));
        }

        // The saved command has been trimmed out of the undo stack, so the
        // save point can never be reached again and must report modified.
        assert!(history.is_modified());
    }

    #[test]
    fn test_set_max_size_trims_multiple_entries_in_one_call() {
        let mut buffer = TextBuffer::new("a");
        let mut cursor = (0, 1);
        let history = CommandHistory::new(10);

        for i in 0..5 {
            let mut cmd = InsertCharCommand::new(0, 1 + i, 'x', cursor);
            cmd.execute(&mut buffer, &mut cursor);
            cursor.1 += 1;
            history.push(Box::new(cmd));
        }
        assert_eq!(history.undo_count(), 5);

        // Shrinking by more than one below the current length must trim
        // more than one command in a single `set_max_size` call.
        history.set_max_size(2);
        assert_eq!(history.undo_count(), 2);
    }

    #[test]
    fn test_push_clears_redo() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);

        let mut cmd1 = InsertCharCommand::new(0, 5, '!', cursor);
        cmd1.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd1));

        history.undo(&mut buffer, &mut cursor);
        assert!(history.can_redo());

        // Push new command should clear redo stack
        let mut cmd2 = InsertCharCommand::new(0, 5, '?', cursor);
        cmd2.execute(&mut buffer, &mut cursor);
        history.push(Box::new(cmd2));

        assert!(!history.can_redo());
    }
}
