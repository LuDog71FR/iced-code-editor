//! Applying an LSP `TextEdit[]` reply to the buffer.
//!
//! Formatting is the first feature to need this, but the shape is the one
//! every edit-returning request uses (code actions, rename, organise
//! imports), so the entry point takes a plain slice of [`LspTextChange`]
//! rather than anything formatting-specific.
//!
//! The edits arrive in document coordinates that all refer to the *same*
//! original document, so they are applied last-first: an earlier edit would
//! otherwise shift the positions of every edit after it.

use crate::buffer::TextBuffer;
use crate::canvas_editor::CodeEditor;
use crate::canvas_editor::editing::command::{
    Command, CompositeCommand, DeleteRangeCommand, InsertTextCommand,
};
use crate::canvas_editor::lsp::{LspPosition, LspTextChange};

/// An LSP edit resolved to in-bounds buffer coordinates.
#[derive(Debug, PartialEq, Eq)]
struct ResolvedEdit {
    /// Start of the replaced range, as `(line, column)`.
    start: (usize, usize),
    /// End of the replaced range, as `(line, column)`.
    end: (usize, usize),
    /// Text replacing that range.
    text: String,
}

/// Clamps an LSP position onto an existing buffer position.
///
/// Servers routinely describe "the end of the document" as the start of a line
/// one past the last one — the shape a whole-document format reply uses. Such a
/// position folds onto the *end* of the last line rather than its start, which
/// is the position it actually denotes; a column past the end of a real line is
/// clamped onto that line's end.
fn clamp_position(
    buffer: &TextBuffer,
    position: LspPosition,
) -> (usize, usize) {
    let last_line = buffer.line_count().saturating_sub(1);
    let line = position.line as usize;
    if line > last_line {
        return (last_line, buffer.line_len(last_line));
    }
    (line, (position.character as usize).min(buffer.line_len(line)))
}

/// Resolves `edits` against `buffer`, dropping the ones that change nothing.
///
/// The result is sorted by start position. Returns `None` when two edits
/// overlap: LSP forbids that, and applying an overlapping pair would silently
/// corrupt the buffer, so the whole reply is refused instead.
fn resolve_edits(
    buffer: &TextBuffer,
    edits: &[LspTextChange],
) -> Option<Vec<ResolvedEdit>> {
    let mut resolved: Vec<ResolvedEdit> = edits
        .iter()
        .map(|edit| {
            let start = clamp_position(buffer, edit.range.start);
            let end = clamp_position(buffer, edit.range.end);
            // A reversed range is not meaningful; normalise rather than
            // trusting the order the server sent the endpoints in.
            let (start, end) =
                if start <= end { (start, end) } else { (end, start) };
            ResolvedEdit { start, end, text: edit.text.clone() }
        })
        .filter(|edit| !(edit.start == edit.end && edit.text.is_empty()))
        .collect();

    resolved.sort_by_key(|edit| edit.start);

    if resolved.windows(2).any(|pair| pair[1].start < pair[0].end) {
        return None;
    }

    Some(resolved)
}

impl CodeEditor {
    /// Applies a batch of LSP text edits to the buffer as one undo step.
    ///
    /// This is how a `TextEdit[]` reply — a `textDocument/formatting`
    /// response, for instance — reaches the document. Positions outside the
    /// buffer are clamped onto it, so an end-of-document range expressed as a
    /// line past the last one still lands correctly.
    ///
    /// The cursor keeps its line and column where the reformatted document
    /// still has one, clamped onto the nearest position where it does not, and
    /// any selection is dropped: after a whole-document rewrite the old
    /// selection no longer covers what the user selected.
    ///
    /// # Arguments
    ///
    /// * `edits` - Edits from the server, all in coordinates of the current document
    ///
    /// # Returns
    ///
    /// `true` when the buffer was modified; `false` when `edits` is empty,
    /// contains only no-op edits, or describes overlapping ranges — which LSP
    /// forbids and which cannot be applied without corrupting the buffer
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, LspPosition, LspRange, LspTextChange};
    ///
    /// let mut editor = CodeEditor::new("fn  main() {}", "rs");
    ///
    /// // The server asks for the double space to become a single one.
    /// let edit = LspTextChange {
    ///     range: LspRange {
    ///         start: LspPosition { line: 0, character: 2 },
    ///         end: LspPosition { line: 0, character: 4 },
    ///     },
    ///     text: " ".to_string(),
    /// };
    ///
    /// assert!(editor.apply_lsp_text_edits(&[edit]));
    /// assert_eq!(editor.content(), "fn main() {}");
    ///
    /// // An empty reply — an already-formatted document — changes nothing.
    /// assert!(!editor.apply_lsp_text_edits(&[]));
    /// ```
    pub fn apply_lsp_text_edits(&mut self, edits: &[LspTextChange]) -> bool {
        let Some(resolved) = resolve_edits(&self.buffer, edits) else {
            return false;
        };
        if resolved.is_empty() {
            return false;
        }

        // A format lands as its own undo step rather than joining whatever
        // typing group happens to be open.
        self.end_grouping_if_active();

        let cursor_before = self.cursors.primary_position();
        let mut composite = CompositeCommand::new();

        // Last edit first: every range is expressed against the document as it
        // stands now, so applying an earlier one would move the ranges of all
        // the edits that follow it.
        for edit in resolved.iter().rev() {
            composite.add(Box::new(DeleteRangeCommand::new(
                &self.buffer,
                edit.start,
                edit.end,
                cursor_before,
            )));
            if !edit.text.is_empty() {
                composite.add(Box::new(InsertTextCommand::new(
                    edit.start.0,
                    edit.start.1,
                    edit.text.clone(),
                    cursor_before,
                )));
            }
        }

        let mut cursor_position = cursor_before;
        composite.execute(&mut self.buffer, &mut cursor_position);
        self.history.push(Box::new(composite));

        self.cursors.set_single(self.clamp_to_buffer(cursor_before));

        // The edits may sit anywhere in the document, so no prefix of the
        // highlight cache can be assumed to have survived them.
        self.pre_edit_line = 0;
        self.pre_edit_last_line = usize::MAX;
        self.finish_edit_operation();
        true
    }

    /// Clamps a `(line, column)` position onto an existing buffer position.
    fn clamp_to_buffer(&self, position: (usize, usize)) -> (usize, usize) {
        let line = position.0.min(self.buffer.line_count().saturating_sub(1));
        (line, position.1.min(self.buffer.line_len(line)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::lsp::LspRange;

    /// Builds an edit replacing `start..end` on a single line with `text`.
    fn edit(start: (u32, u32), end: (u32, u32), text: &str) -> LspTextChange {
        LspTextChange {
            range: LspRange {
                start: LspPosition { line: start.0, character: start.1 },
                end: LspPosition { line: end.0, character: end.1 },
            },
            text: text.to_string(),
        }
    }

    #[test]
    fn test_clamp_position_keeps_an_in_bounds_position() {
        let buffer = TextBuffer::new("abc\ndef");
        assert_eq!(
            clamp_position(&buffer, LspPosition { line: 1, character: 2 }),
            (1, 2)
        );
    }

    #[test]
    fn test_clamp_position_folds_a_past_the_end_position_onto_the_document() {
        let buffer = TextBuffer::new("abc\ndef");
        // The shape servers use for "the very end of the document".
        assert_eq!(
            clamp_position(
                &buffer,
                LspPosition { line: u32::MAX, character: 0 }
            ),
            (1, 3)
        );
        assert_eq!(
            clamp_position(
                &buffer,
                LspPosition { line: 0, character: u32::MAX }
            ),
            (0, 3)
        );
    }

    #[test]
    fn test_resolve_edits_sorts_and_drops_no_ops() {
        let buffer = TextBuffer::new("abcdef");
        let resolved = resolve_edits(
            &buffer,
            &[
                edit((0, 4), (0, 5), "Y"),
                edit((0, 2), (0, 2), ""),
                edit((0, 0), (0, 1), "X"),
            ],
        );

        assert_eq!(
            resolved,
            Some(vec![
                ResolvedEdit {
                    start: (0, 0),
                    end: (0, 1),
                    text: "X".to_string()
                },
                ResolvedEdit {
                    start: (0, 4),
                    end: (0, 5),
                    text: "Y".to_string()
                },
            ])
        );
    }

    #[test]
    fn test_resolve_edits_refuses_overlapping_ranges() {
        let buffer = TextBuffer::new("abcdef");
        assert_eq!(
            resolve_edits(
                &buffer,
                &[edit((0, 0), (0, 3), "X"), edit((0, 2), (0, 4), "Y")]
            ),
            None
        );
    }

    #[test]
    fn test_resolve_edits_accepts_ranges_that_only_touch() {
        let buffer = TextBuffer::new("abcdef");
        let resolved = resolve_edits(
            &buffer,
            &[edit((0, 0), (0, 2), "X"), edit((0, 2), (0, 4), "Y")],
        );
        assert!(resolved.is_some_and(|edits| edits.len() == 2));
    }

    #[test]
    fn test_apply_replaces_the_whole_document() {
        let mut editor = CodeEditor::new("fn  main( ) {}", "rs");
        let applied = editor.apply_lsp_text_edits(&[edit(
            (0, 0),
            (1, 0),
            "fn main() {}",
        )]);

        assert!(applied);
        assert_eq!(editor.content(), "fn main() {}");
    }

    #[test]
    fn test_apply_several_edits_uses_the_original_coordinates() {
        // Both ranges describe the document as it stands *before* either edit,
        // so applying the first must not shift the second.
        let mut editor = CodeEditor::new("aXbYc", "rs");
        let applied = editor.apply_lsp_text_edits(&[
            edit((0, 1), (0, 2), "1"),
            edit((0, 3), (0, 4), "22"),
        ]);

        assert!(applied);
        assert_eq!(editor.content(), "a1b22c");
    }

    #[test]
    fn test_apply_across_lines_reflows_the_document() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        let applied =
            editor.apply_lsp_text_edits(&[edit((0, 3), (2, 0), "\n")]);

        assert!(applied);
        assert_eq!(editor.content(), "one\nthree");
    }

    #[test]
    fn test_apply_is_a_single_undo_step() {
        let mut editor = CodeEditor::new("a\nb\nc", "rs");
        let applied = editor.apply_lsp_text_edits(&[
            edit((0, 0), (0, 1), "A"),
            edit((2, 0), (2, 1), "C"),
        ]);
        assert!(applied);
        assert_eq!(editor.content(), "A\nb\nC");

        let _ = editor.update(&crate::canvas_editor::Message::Undo);
        assert_eq!(editor.content(), "a\nb\nc");
    }

    #[test]
    fn test_apply_keeps_the_cursor_where_it_was() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        let _ = editor.set_cursor(1, 2);
        assert!(editor.apply_lsp_text_edits(&[edit((0, 0), (0, 3), "1")]));
        assert_eq!(editor.cursor_position(), (1, 2));
    }

    #[test]
    fn test_apply_clamps_a_cursor_the_edits_left_out_of_bounds() {
        let mut editor = CodeEditor::new("one\ntwo\nthree", "rs");
        let _ = editor.set_cursor(2, 5);
        // The document shrinks to a single line under the cursor's position.
        assert!(editor.apply_lsp_text_edits(&[edit((0, 0), (2, 5), "x")]));
        assert_eq!(editor.cursor_position(), (0, 1));
    }

    #[test]
    fn test_apply_reports_nothing_done_for_an_empty_reply() {
        let mut editor = CodeEditor::new("fn main() {}", "rs");
        assert!(!editor.apply_lsp_text_edits(&[]));
        assert_eq!(editor.content(), "fn main() {}");
    }

    #[test]
    fn test_apply_reports_nothing_done_for_overlapping_edits() {
        let mut editor = CodeEditor::new("abcdef", "rs");
        assert!(!editor.apply_lsp_text_edits(&[
            edit((0, 0), (0, 3), "X"),
            edit((0, 2), (0, 4), "Y"),
        ]));
        assert_eq!(editor.content(), "abcdef");
    }

    #[test]
    fn test_apply_marks_the_document_modified() {
        let mut editor = CodeEditor::new("fn  main() {}", "rs");
        assert!(!editor.is_modified());
        assert!(editor.apply_lsp_text_edits(&[edit((0, 2), (0, 4), " ")]));
        assert!(editor.is_modified());
    }
}
