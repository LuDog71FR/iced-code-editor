//! Message handling and update logic.

use crate::canvas_editor::{
    CodeEditor, LspEditSnapshot, Message, VimMode, cursor_set, lsp,
};

// =========================================================================
// Cursor adjustment helpers for multi-cursor editing
// =========================================================================

/// Describes the kind of edit applied to a single position.
#[derive(Clone, Copy)]
pub(crate) enum EditType {
    /// Insert one char at `(edit_line, edit_col)`.
    InsertChar,
    /// Backspace: delete char at `(edit_line, edit_col - 1)`.
    DeleteCharBack,
    /// Delete-forward: delete char at `(edit_line, edit_col)`.
    DeleteCharForward,
    /// Enter: split `edit_line` at `edit_col`; new line has `extra` indent chars.
    InsertNewline { indent_len: usize },
    /// Backspace-at-col-0: merge `edit_line` into `edit_line - 1`.
    /// `extra` = length of the previous line before merge.
    MergePrev { prev_line_len: usize },
    /// Delete-at-end-of-line: merge `edit_line + 1` into `edit_line`.
    /// `extra` = length of `edit_line` before merge.
    MergeNext { edit_line_len: usize },
}

/// Adjusts a single `(line, col)` pair after an edit.
pub(crate) fn adjust_pos(
    pos: &mut (usize, usize),
    edit_line: usize,
    edit_col: usize,
    kind: EditType,
) {
    match kind {
        EditType::InsertChar => {
            if pos.0 == edit_line && pos.1 >= edit_col {
                pos.1 += 1;
            }
        }
        EditType::DeleteCharBack => {
            if edit_col > 0 && pos.0 == edit_line && pos.1 > edit_col - 1 {
                pos.1 -= 1;
            }
        }
        EditType::DeleteCharForward => {
            if pos.0 == edit_line && pos.1 > edit_col {
                pos.1 -= 1;
            }
        }
        EditType::InsertNewline { indent_len } => {
            if pos.0 > edit_line {
                pos.0 += 1;
            } else if pos.0 == edit_line && pos.1 >= edit_col {
                pos.0 += 1;
                pos.1 = pos.1 - edit_col + indent_len;
            }
        }
        EditType::MergePrev { prev_line_len } => {
            if pos.0 == edit_line {
                pos.0 -= 1;
                pos.1 += prev_line_len;
            } else if pos.0 > edit_line {
                pos.0 -= 1;
            }
        }
        EditType::MergeNext { edit_line_len } => {
            if pos.0 == edit_line + 1 {
                pos.0 = edit_line;
                pos.1 += edit_line_len;
            } else if pos.0 > edit_line + 1 {
                pos.0 -= 1;
            }
        }
    }
}

/// Adjusts all cursors except `skip_idx` after an edit at `(edit_line, edit_col)`.
pub(crate) fn adjust_other_cursors(
    cursors: &mut [cursor_set::Cursor],
    skip_idx: usize,
    edit_line: usize,
    edit_col: usize,
    kind: EditType,
) {
    for (i, cursor) in cursors.iter_mut().enumerate() {
        if i == skip_idx {
            continue;
        }
        adjust_pos(&mut cursor.position, edit_line, edit_col, kind);
        if let Some(ref mut anchor) = cursor.anchor {
            adjust_pos(anchor, edit_line, edit_col, kind);
        }
    }
}

impl CodeEditor {
    // =========================================================================
    // Helper Methods
    // =========================================================================

    /// Performs common cleanup operations after edit operations.
    ///
    /// This method should be called after any operation that modifies the buffer content.
    /// It resets the cursor blink animation, refreshes search matches if search is active,
    /// and invalidates all caches that depend on buffer content or layout:
    /// - `buffer_revision` is bumped to invalidate layout-derived caches
    /// - `visual_lines_cache` is cleared so wrapping is recalculated on next use
    /// - `content_cache` and `overlay_cache` are cleared to rebuild canvas geometry
    pub(crate) fn finish_edit_operation(&mut self) {
        self.reset_cursor_blink();
        self.refresh_search_matches_if_needed();
        // The exact revision value is not semantically meaningful; it only needs
        // to change on edits, so `wrapping_add` is sufficient and overflow-safe.
        let previous_revision = self.buffer_revision;
        self.buffer_revision = self.buffer_revision.wrapping_add(1);
        self.refresh_visual_lines_after_edit(previous_revision);
        self.refresh_max_content_width_after_edit(previous_revision);
        // Truncate the syntax-highlight cache from the first line the edit may
        // have changed. `pre_edit_line` is the topmost active line captured
        // before the edit; the extra line of margin covers edits that merge
        // with the preceding line (e.g. backspace at column 0).
        self.invalidate_highlight_from(self.pre_edit_line.saturating_sub(1));
        self.bracket_depth_cache
            .borrow_mut()
            .truncate_from(self.pre_edit_line.saturating_sub(1));
        self.content_cache.clear();
        self.overlay_cache.clear();
        self.enqueue_incremental_lsp_change();
    }

    /// Returns the topmost logical line currently touched by any cursor or its
    /// selection anchor.
    ///
    /// This is captured before an edit to bound which highlight-cache lines may
    /// change. With no cursors it defaults to line `0`.
    pub(crate) fn min_active_line(&self) -> usize {
        self.cursors
            .iter()
            .map(|cursor| match cursor.anchor {
                Some(anchor) => cursor.position.0.min(anchor.0),
                None => cursor.position.0,
            })
            .min()
            .unwrap_or(0)
    }

    /// Returns the bottommost logical line touched by a cursor or selection.
    fn max_active_line(&self) -> usize {
        self.cursors
            .iter()
            .map(|cursor| match cursor.anchor {
                Some(anchor) => cursor.position.0.max(anchor.0),
                None => cursor.position.0,
            })
            .max()
            .unwrap_or(0)
    }

    /// Captures a conservative old-document line range for an incremental LSP
    /// replacement. Non-editing messages do not allocate or retain a snapshot.
    pub(crate) fn capture_lsp_edit_snapshot(&mut self, message: &Message) {
        if self.lsp_document.is_none() {
            self.lsp_edit_snapshot = None;
            return;
        }

        let is_local_edit = matches!(
            message,
            Message::CharacterInput(_)
                | Message::Tab
                | Message::Enter
                | Message::Backspace
                | Message::Delete
                | Message::DeleteSelection
                | Message::Paste(_)
                | Message::ImeCommit(_)
                | Message::MoveLineUp
                | Message::MoveLineDown
                | Message::DuplicateLineUp
                | Message::DuplicateLineDown
                | Message::ToggleComment
        );
        let is_global_edit = matches!(
            message,
            Message::Undo | Message::Redo | Message::ReplaceAll
        );
        let is_replace_next = matches!(message, Message::ReplaceNext);
        if !is_local_edit && !is_global_edit && !is_replace_next {
            self.lsp_edit_snapshot = None;
            return;
        }

        let line_count = self.buffer.line_count();
        let (mut first_line, mut last_line) = if is_global_edit {
            (0, line_count.saturating_sub(1))
        } else {
            (self.pre_edit_line, self.pre_edit_last_line)
        };
        if is_replace_next
            && let Some(search_match) = self.search_state.current_match()
        {
            first_line = first_line.min(search_match.line);
            last_line = last_line.max(search_match.line);
        }

        let start_line =
            if is_global_edit { 0 } else { first_line.saturating_sub(1) };
        let old_end_exclusive = if is_global_edit {
            line_count
        } else {
            last_line.saturating_add(2).min(line_count)
        };
        let old_end = if old_end_exclusive < line_count {
            lsp::LspPosition {
                line: u32::try_from(old_end_exclusive).unwrap_or(u32::MAX),
                character: 0,
            }
        } else {
            let last_line = line_count.saturating_sub(1);
            lsp::LspPosition {
                line: u32::try_from(last_line).unwrap_or(u32::MAX),
                character: u32::try_from(self.buffer.line_len(last_line))
                    .unwrap_or(u32::MAX),
            }
        };

        self.lsp_edit_snapshot = Some(LspEditSnapshot {
            start_line,
            old_end_exclusive,
            old_line_count: line_count,
            old_end,
        });
    }

    /// Truncates the syntax-highlight cache so logical lines `>= line` are
    /// re-highlighted on next access.
    ///
    /// Lines before the first edited line are unaffected, so the cached prefix
    /// is preserved and edits never trigger a full re-parse from the top of the
    /// file. Has no effect when the cache is empty.
    ///
    /// # Arguments
    ///
    /// * `line` - First logical line to invalidate.
    pub(crate) fn invalidate_highlight_from(&self, line: usize) {
        if let Some(cache) = self.highlight_cache.borrow_mut().as_mut() {
            cache.truncate(line);
        }
    }

    /// Performs common cleanup operations after navigation operations.
    ///
    /// This method should be called after cursor movement operations.
    /// It resets the cursor blink animation and invalidates only the overlay
    /// rendering cache. Cursor movement and selection changes do not modify the
    /// buffer content, so keeping the content cache intact avoids unnecessary
    /// re-rendering of syntax-highlighted text.
    pub(crate) fn finish_navigation_operation(&mut self) {
        self.sync_search_match_from_primary_cursor();
        self.reset_cursor_blink();
        self.overlay_cache.clear();
    }

    /// Starts command grouping if not already grouping.
    ///
    /// This is used for smart undo functionality, allowing multiple related
    /// operations to be undone as a single unit.
    pub(crate) fn ensure_grouping_started(&mut self) {
        if !self.is_grouping {
            self.history.begin_group();
            self.is_grouping = true;
        }
    }

    /// Ends command grouping if currently active.
    ///
    /// This should be called when a series of related operations is complete,
    /// or when starting a new type of operation that shouldn't be grouped
    /// with previous operations.
    pub(crate) fn end_grouping_if_active(&mut self) {
        if self.is_grouping {
            self.history.end_group();
            self.is_grouping = false;
        }
    }

    fn keep_vim_insert_group(&self) -> bool {
        self.vim_enabled
            && self.vim_state.mode() == VimMode::Insert
            && self.is_grouping
    }

    /// Deletes all active selections across every cursor and performs cleanup.
    ///
    /// When more than one cursor holds a selection, every cursor's deletion
    /// is grouped into one composite command so a single undo restores all
    /// of them, instead of requiring one undo per cursor. A lone selection
    /// is left ungrouped, as before: some callers (e.g. backspace during a
    /// Vim insert-mode session) rely on grouping state carrying across this
    /// call, and a single deletion doesn't need its own group to already
    /// undo as one step.
    ///
    /// # Returns
    ///
    /// `true` if at least one selection was deleted, `false` if no cursor had a selection
    fn delete_selection_if_present(&mut self) -> bool {
        let selection_count =
            self.cursors.iter().filter(|c| c.has_selection()).count();
        if selection_count == 0 {
            return false;
        }

        let multi = selection_count > 1;
        if multi {
            self.ensure_grouping_started();
        }
        self.delete_selection();
        if multi {
            self.end_grouping_if_active();
        }
        self.finish_edit_operation();
        true
    }
}

mod clipboard;
mod deletion;
mod dispatch;
mod focus_ime;
mod history_ops;
mod line_ops;
mod mouse;
mod multi_cursor;
mod navigation;
mod scroll_timer;
mod text_input;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_increments_revision_and_clears_visual_lines_cache() {
        let mut editor = CodeEditor::new("hello", "rs");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (0, 5);

        let _ = editor.visual_lines_cached(800.0);
        assert!(
            editor.visual_lines_cache.borrow().is_some(),
            "visual_lines_cached should populate the cache"
        );

        let previous_revision = editor.buffer_revision;

        let _ = editor.update(&Message::CharacterInput('!'));
        assert_eq!(
            editor.buffer_revision,
            previous_revision.wrapping_add(1),
            "buffer_revision should change on buffer edits"
        );
        // `scroll_to_cursor` repopulates the cache after the edit with the new
        // revision, so the cache may be `Some`.  What must never happen is that
        // stale data (an old revision) survives an edit.
        assert!(
            editor
                .visual_lines_cache
                .borrow()
                .as_ref()
                .is_none_or(|c| c.key.buffer_revision == editor.buffer_revision),
            "buffer edits should not leave stale data in the visual lines cache"
        );
    }

    #[test]
    fn test_edit_refreshes_only_affected_search_matches() {
        let mut editor = CodeEditor::new("foo\nfoo\nfoo", "rs");
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.search_state.open_search();
        editor.search_state.set_query("foo".to_owned(), &editor.buffer);
        editor.cursors.primary_mut().position = (1, 1);

        let _ = editor.update(&Message::CharacterInput('x'));

        let match_lines: Vec<usize> =
            editor.search_state.matches.iter().map(|item| item.line).collect();
        assert_eq!(match_lines, vec![0, 2]);
    }

    #[test]
    fn test_incremental_visual_lines_match_full_recalculation_after_newline() {
        use std::collections::HashSet;

        let mut editor = CodeEditor::new("zero\nabcdefgh\nlast", "rs")
            .with_wrap_column(Some(4));
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (1, 4);

        let _ = editor.visual_lines_cached(800.0);
        let _ = editor.update(&Message::Enter);
        let incremental = editor.visual_lines_cached(800.0);

        let calculator =
            crate::canvas_editor::render::wrapping::WrappingCalculator::new(
                editor.wrap_enabled,
                editor.wrap_column,
                editor.full_char_width,
                editor.char_width,
            );
        let expected = calculator.calculate_visual_lines(
            &editor.buffer,
            800.0,
            editor.gutter_width(),
            &HashSet::new(),
        );

        assert_eq!(incremental.as_ref(), &expected);
    }
}
