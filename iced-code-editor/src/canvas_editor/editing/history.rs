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

use super::command::{Command, CompositeCommand};
use crate::buffer::TextBuffer;
use crate::canvas_editor::CodeEditor;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

/// Manages command history for undo/redo operations.
///
/// The history maintains two stacks:
/// - Undo stack: Commands that can be undone
/// - Redo stack: Commands that can be redone (cleared when new commands are added)
///
/// Thread-safe using Arc<Mutex<>> for interior mutability. Cloning shares the
/// same history: every clone reads and writes the one set of stacks.
///
/// No method on this type panics on a poisoned mutex: a poisoned lock is
/// recovered rather than propagated, so a panic inside one caller's `Command`
/// cannot leave the history permanently unusable for everyone else.
///
/// Each [`CodeEditor`] owns one of these and records every edit into it, so
/// most applications observe it through [`CodeEditor::can_undo`],
/// [`CodeEditor::is_modified`], and the [`Message::Undo`] / [`Message::Redo`]
/// messages rather than constructing one directly.
///
/// # Example
///
/// ```
/// use iced_code_editor::{CodeEditor, Message};
///
/// let mut editor = CodeEditor::new("hello", "rs");
/// assert!(!editor.can_undo());
///
/// let _ = editor.update(&Message::Paste(" world".to_string()));
/// assert!(editor.can_undo());
/// assert!(editor.is_modified());
///
/// let _ = editor.update(&Message::Undo);
/// assert_eq!(editor.content(), "hello");
/// assert!(editor.can_redo());
/// ```
///
/// [`Message::Undo`]: crate::Message::Undo
/// [`Message::Redo`]: crate::Message::Redo
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
    /// Locks the shared state, recovering the guard if the mutex was poisoned.
    ///
    /// Poisoning is reachable here without a second thread. [`Self::undo`] and
    /// [`Self::redo`] run `Command::undo` / `Command::execute` *while holding
    /// this guard*, and [`Command`] is a public trait: a panic inside a
    /// caller-supplied implementation poisons the mutex on this very thread.
    /// Panicking again on every subsequent lock would turn one recoverable
    /// mistake into a dead editor, because the render path calls
    /// [`Self::can_undo`] on each frame and every [`Clone`] of this handle
    /// shares the same poisoned mutex.
    ///
    /// Taking the guard is safe to do here: the only invariant a poisoned
    /// `HistoryInner` can break is "the undo stack matches the buffer", which a
    /// caller recovers from with [`Self::clear`]. This matches the policy the
    /// LSP client applies to its own shared state.
    fn lock_inner(&self) -> MutexGuard<'_, HistoryInner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

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
    /// The `Command` trait is internal to this crate, so an application does
    /// not call this directly — [`CodeEditor`] pushes a command for each edit
    /// it performs. The example below shows the observable effect.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to add
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    ///
    /// // Each edit the editor performs pushes a command.
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    /// assert!(editor.can_undo());
    ///
    /// // Pushing after an undo discards the redo stack: the alternative
    /// // future is gone once a new edit diverges from it.
    /// let _ = editor.update(&Message::Undo);
    /// assert!(editor.can_redo());
    /// let _ = editor.update(&Message::Paste("?".to_string()));
    /// assert!(!editor.can_redo());
    /// ```
    pub fn push(&self, command: Box<dyn Command>) {
        let mut inner = self.lock_inner();

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
    /// Any open group is closed first, so an in-progress run of typing undoes
    /// as one unit rather than being left half-open.
    ///
    /// `TextBuffer` is internal to this crate, so an application drives this
    /// through [`Message::Undo`] rather than calling it directly.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to modify
    /// * `cursor` - The cursor position to update
    ///
    /// # Returns
    ///
    /// `true` if a command was undone, `false` if nothing to undo
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// let _ = editor.update(&Message::Paste(" world".to_string()));
    /// assert_eq!(editor.content(), " worldhello");
    ///
    /// let _ = editor.update(&Message::Undo);
    /// assert_eq!(editor.content(), "hello");
    ///
    /// // Undoing with an empty stack is a harmless no-op.
    /// let _ = editor.update(&Message::Undo);
    /// assert_eq!(editor.content(), "hello");
    /// ```
    ///
    /// [`Message::Undo`]: crate::Message::Undo
    pub fn undo(
        &self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) -> bool {
        let mut inner = self.lock_inner();

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
    /// `TextBuffer` is internal to this crate, so an application drives this
    /// through [`Message::Redo`] rather than calling it directly.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer to modify
    /// * `cursor` - The cursor position to update
    ///
    /// # Returns
    ///
    /// `true` if a command was redone, `false` if nothing to redo
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// let _ = editor.update(&Message::Paste(" world".to_string()));
    /// let _ = editor.update(&Message::Undo);
    /// assert_eq!(editor.content(), "hello");
    ///
    /// let _ = editor.update(&Message::Redo);
    /// assert_eq!(editor.content(), " worldhello");
    /// ```
    ///
    /// [`Message::Redo`]: crate::Message::Redo
    pub fn redo(
        &self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) -> bool {
        let mut inner = self.lock_inner();

        if let Some(mut command) = inner.redo_stack.pop() {
            command.execute(buffer, cursor);
            inner.undo_stack.push_back(command);
            true
        } else {
            false
        }
    }

    /// Returns whether there are commands that can be undone.
    ///
    /// An open group counts as undoable even before it is closed, so an
    /// "Undo" menu item stays enabled while the user is mid-word.
    ///
    /// # Returns
    ///
    /// `true` if undo would do something, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// assert!(!history.can_undo());
    /// ```
    #[must_use]
    pub fn can_undo(&self) -> bool {
        let inner = self.lock_inner();
        !inner.undo_stack.is_empty() || inner.current_group.is_some()
    }

    /// Returns whether there are commands that can be redone.
    ///
    /// # Returns
    ///
    /// `true` if redo would do something, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// assert!(!editor.can_redo());
    ///
    /// // Redo only becomes available after an undo.
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    /// assert!(!editor.can_redo());
    /// let _ = editor.update(&Message::Undo);
    /// assert!(editor.can_redo());
    /// ```
    #[must_use]
    pub fn can_redo(&self) -> bool {
        let inner = self.lock_inner();
        !inner.redo_stack.is_empty()
    }

    /// Marks the current position as the save point.
    ///
    /// This is used to track whether the document has been modified since
    /// the last save. Call this after successfully saving the file.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    /// assert!(editor.is_modified());
    ///
    /// // After the host has written the file to disk.
    /// editor.mark_saved();
    /// assert!(!editor.is_modified());
    ///
    /// // Undoing back past the save point marks the document dirty again.
    /// let _ = editor.update(&Message::Undo);
    /// assert!(editor.is_modified());
    /// ```
    pub fn mark_saved(&self) {
        let mut inner = self.lock_inner();
        inner.save_point = Some(inner.undo_stack.len());
    }

    /// Returns whether the document has been modified since the last save.
    ///
    /// Compares the current undo depth against the depth recorded by
    /// [`Self::mark_saved`], so undoing back to the saved state reports "not
    /// modified" again rather than staying permanently dirty.
    ///
    /// # Returns
    ///
    /// `true` if there are unsaved changes, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    /// // A fresh history has nothing to save.
    /// assert!(!history.is_modified());
    /// ```
    #[must_use]
    pub fn is_modified(&self) -> bool {
        let inner = self.lock_inner();

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
        let mut inner = self.lock_inner();
        inner.undo_stack.clear();
        inner.redo_stack.clear();
        inner.save_point = None;
        inner.current_group = None;
    }

    /// Begins grouping subsequent commands into a composite.
    ///
    /// All commands added via `push()` will be grouped together until
    /// `end_group()` is called. This is useful for grouping consecutive
    /// typing operations, so one undo removes a whole word rather than one
    /// character at a time.
    ///
    /// Calling this while a group is already open does nothing, so grouping
    /// does not nest.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    ///
    /// history.begin_group();
    /// // An open group counts as undoable, so an "Undo" menu item stays
    /// // enabled while the user is still typing.
    /// assert!(history.can_undo());
    ///
    /// // Nothing was actually pushed, so closing it adds no entry.
    /// history.end_group();
    /// assert_eq!(history.undo_count(), 0);
    /// ```
    pub fn begin_group(&self) {
        let mut inner = self.lock_inner();
        if inner.current_group.is_none() {
            inner.current_group = Some(CompositeCommand::new());
        }
    }

    /// Ends the current command grouping.
    ///
    /// The grouped commands are added to the history as a single composite
    /// command, so one undo reverses all of them. If no commands were grouped,
    /// nothing is added — an empty group leaves no trace.
    ///
    /// Safe to call when no group is open.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CommandHistory;
    ///
    /// let history = CommandHistory::new(100);
    ///
    /// history.begin_group();
    /// history.end_group();
    /// // An empty group adds nothing to the undo stack.
    /// assert_eq!(history.undo_count(), 0);
    /// assert!(!history.can_undo());
    ///
    /// // Closing when nothing is open is harmless.
    /// history.end_group();
    /// ```
    pub fn end_group(&self) {
        let mut inner = self.lock_inner();
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
        let inner = self.lock_inner();
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
        let mut inner = self.lock_inner();
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
        let inner = self.lock_inner();
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
        let inner = self.lock_inner();
        inner.redo_stack.len()
    }
}

// Implement Default for convenient usage
impl Default for CommandHistory {
    fn default() -> Self {
        Self::new(100)
    }
}

impl CodeEditor {
    /// Returns whether the editor has unsaved changes.
    ///
    /// Use this to drive a "modified" indicator in a tab title, or to prompt
    /// before closing.
    ///
    /// # Returns
    ///
    /// `true` if there are unsaved modifications, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// assert!(!editor.is_modified());
    ///
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    /// assert!(editor.is_modified());
    /// ```
    pub fn is_modified(&self) -> bool {
        self.history.is_modified()
    }

    /// Marks the current state as saved.
    ///
    /// Call this after successfully saving the file to reset the modified state.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    ///
    /// editor.mark_saved();
    /// assert!(!editor.is_modified());
    /// ```
    pub fn mark_saved(&mut self) {
        self.history.mark_saved();
    }

    /// Returns whether undo is available.
    ///
    /// # Returns
    ///
    /// `true` if undo would do something, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// assert!(!editor.can_undo());
    ///
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    /// assert!(editor.can_undo());
    /// ```
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Returns whether redo is available.
    ///
    /// # Returns
    ///
    /// `true` if redo would do something, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Message};
    ///
    /// let mut editor = CodeEditor::new("hello", "rs");
    /// let _ = editor.update(&Message::Paste("!".to_string()));
    /// assert!(!editor.can_redo());
    ///
    /// let _ = editor.update(&Message::Undo);
    /// assert!(editor.can_redo());
    /// ```
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
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

    /// A caller-supplied command whose `undo` panics, standing in for a buggy
    /// [`Command`] implementation in a downstream crate.
    ///
    /// The panic is raised by an out-of-bounds index rather than `panic!`,
    /// which is denied workspace-wide.
    #[derive(Debug)]
    struct PanickingUndoCommand;

    impl Command for PanickingUndoCommand {
        fn execute(
            &mut self,
            _buffer: &mut TextBuffer,
            _cursor: &mut (usize, usize),
        ) {
        }

        fn undo(
            &mut self,
            _buffer: &mut TextBuffer,
            _cursor: &mut (usize, usize),
        ) {
            let empty: Vec<u8> = Vec::new();
            let _ = empty[0];
        }
    }

    /// A panic inside a caller's `Command` poisons the history mutex on this
    /// very thread, because `undo` runs the command while holding the guard.
    /// Every later call must still work: `can_undo` is on the render path, so
    /// panicking again there would take the whole editor down permanently.
    ///
    /// Compiled only under `panic = "unwind"`; the release profile sets
    /// `panic = "abort"`, where the panic below cannot be caught at all.
    #[test]
    #[cfg(panic = "unwind")]
    fn test_history_survives_a_panicking_command() {
        let mut buffer = TextBuffer::new("hello");
        let mut cursor = (0, 5);
        let history = CommandHistory::new(10);
        history.push(Box::new(PanickingUndoCommand));
        assert_eq!(history.undo_count(), 1);

        // Swallow the panic message so the test output stays readable, then
        // restore the default hook for the rest of the suite.
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                history.undo(&mut buffer, &mut cursor)
            }));
        std::panic::set_hook(previous_hook);
        assert!(outcome.is_err(), "the command under test must panic");

        // The mutex is now poisoned. Every read must still answer rather than
        // panic, and the state is the one the unwind left behind: `undo` had
        // already popped the command off the undo stack, and never reached the
        // push onto the redo stack.
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo_count(), 0);
        assert_eq!(history.redo_count(), 0);
        assert!(!history.is_modified());

        // The history must also remain usable, not merely readable.
        history.clear();

        let mut command = InsertCharCommand::new(0, 5, '!', cursor);
        command.execute(&mut buffer, &mut cursor);
        history.push(Box::new(command));
        assert!(history.undo(&mut buffer, &mut cursor));
        assert_eq!(buffer.to_string(), "hello");
    }
}
