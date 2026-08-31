//! The client's mirror of each open document.
//!
//! LSP positions are UTF-16 column offsets, while the editor works in
//! characters, so the client keeps its own copy of every open document purely
//! to translate between the two. The mirror is advanced by the same changes
//! that are sent to the server; if it ever drifts, position translation is no
//! longer trustworthy and the caller must reseed it — see
//! [`apply_changes_to_document`].

use serde_json::json;

use crate::buffer::text_utils::char_to_byte_index;
use crate::canvas_editor::lsp::{LspPosition, LspTextChange};

/// Internal representation of a text document as a vector of lines.
///
/// Used to track document state and convert between character and byte indices.
pub(super) struct TextModel {
    /// The document content stored as a vector of lines (without newline characters)
    lines: Vec<String>,
}

impl TextModel {
    /// Creates a new `TextModel` from a string.
    ///
    /// Splits the text into lines for easier manipulation.
    /// An empty string creates a single empty line.
    pub(super) fn from_text(text: &str) -> Self {
        let lines = if text.is_empty() {
            vec![String::new()]
        } else {
            text.lines().map(String::from).collect()
        };
        Self { lines }
    }

    /// Applies a text change (edit) to the document.
    ///
    /// Handles multi-line insertions and deletions by splicing the lines vector.
    ///
    /// Returns `false` without modifying `self` when `change`'s range falls
    /// outside the current line count. That means this mirror has drifted
    /// out of sync with the real document (e.g. a change was computed
    /// against a state this mirror never saw) — the caller must not trust
    /// this `TextModel` for further position translation once that happens.
    fn apply_change(&mut self, change: &LspTextChange) -> bool {
        let start_line = change.range.start.line as usize;
        let end_line = change.range.end.line as usize;

        if start_line >= self.lines.len() || end_line >= self.lines.len() {
            return false;
        }

        let start_col = change.range.start.character as usize;
        let end_col = change.range.end.character as usize;

        let start_byte = char_to_byte_index(&self.lines[start_line], start_col);
        let end_byte = char_to_byte_index(&self.lines[end_line], end_col);

        let prefix = self.lines[start_line][..start_byte].to_string();
        let suffix = self.lines[end_line][end_byte..].to_string();

        let inserted: Vec<&str> = change.text.split('\n').collect();
        let mut replacement: Vec<String> = Vec::new();

        if inserted.len() == 1 {
            replacement.push(format!("{}{}{}", prefix, inserted[0], suffix));
        } else {
            replacement.push(format!("{}{}", prefix, inserted[0]));
            for mid in inserted.iter().take(inserted.len() - 1).skip(1) {
                replacement.push((*mid).to_string());
            }
            replacement.push(format!(
                "{}{}",
                inserted[inserted.len() - 1],
                suffix
            ));
        }

        self.lines.splice(start_line..=end_line, replacement);
        true
    }

    /// Converts a UTF-8 character position to a UTF-16 position.
    ///
    /// This is necessary because LSP uses UTF-16 for character positions.
    pub(super) fn to_utf16_position(
        &self,
        position: LspPosition,
    ) -> LspPosition {
        let line_index = position.line as usize;
        let char_index = position.character as usize;
        let line = self.lines.get(line_index).map_or("", |l| l.as_str());

        let utf16_col =
            line.chars().take(char_index).map(|c| c.len_utf16() as u32).sum();
        LspPosition { line: position.line, character: utf16_col }
    }

    /// Converts a UTF-16 position back to a character position.
    ///
    /// The inverse of [`Self::to_utf16_position`], needed for every reply
    /// whose payload is expressed in document coordinates (formatting edits):
    /// the editor addresses text by character, so a server column counted in
    /// UTF-16 code units would land in the wrong place on any line holding a
    /// non-ASCII character.
    ///
    /// A column past the end of the line yields that line's character count,
    /// and a column landing inside a surrogate pair rounds up to the end of
    /// the character containing it — neither is a position the editor can
    /// address on its own.
    pub(super) fn to_char_position(
        &self,
        position: LspPosition,
    ) -> LspPosition {
        let line = self
            .lines
            .get(position.line as usize)
            .map_or("", |line| line.as_str());

        let mut utf16_col = 0u32;
        let mut char_col = 0u32;
        for character in line.chars() {
            if utf16_col >= position.character {
                break;
            }
            utf16_col += character.len_utf16() as u32;
            char_col += 1;
        }

        LspPosition { line: position.line, character: char_col }
    }
}

// =============================================================================
// Document State - Tracks the state of an open document
// =============================================================================

/// Represents the state of a single open document.
pub(super) struct DocumentState {
    /// The text content of the document
    pub(super) text: TextModel,
}

/// Applies `changes` to `state`'s mirror in order, converting each to the
/// UTF-16 JSON shape LSP's `didChange` notification expects.
///
/// Returns `None` — instead of the changes converted so far — the moment any
/// change's range falls outside the mirror. That means the mirror has
/// already drifted out of sync with the real document, so every change from
/// that point on (their coordinates are relative to the mirror's state after
/// prior changes) is computed against a state the mirror doesn't actually
/// have; forwarding a partial or best-effort batch would tell the server
/// something that isn't true. The caller is responsible for treating the
/// document as stale (see [`LspProcessClient::apply_change_and_convert`]).
pub(super) fn apply_changes_to_document(
    state: &mut DocumentState,
    changes: &[LspTextChange],
) -> Option<Vec<serde_json::Value>> {
    let mut out = Vec::with_capacity(changes.len());
    for change in changes {
        let start = state.text.to_utf16_position(change.range.start);
        let end = state.text.to_utf16_position(change.range.end);

        if !state.text.apply_change(change) {
            return None;
        }

        out.push(json!({
            "range": {
                "start": { "line": start.line, "character": start.character },
                "end": { "line": end.line, "character": end.character }
            },
            "text": change.text
        }));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::lsp::LspRange;

    #[test]
    fn test_to_char_position_is_the_inverse_on_a_multibyte_line() {
        // "héllo" and "汉字" are one UTF-16 unit per character; the emoji is
        // two, which is where the two coordinate systems come apart.
        let model = TextModel::from_text("héllo\n汉字\n🚀ab");

        for (line, char_col, utf16_col) in
            [(0u32, 3u32, 3u32), (1, 2, 2), (2, 1, 2), (2, 3, 4)]
        {
            let position = LspPosition { line, character: char_col };
            let utf16 = model.to_utf16_position(position);
            assert_eq!(utf16.character, utf16_col);
            assert_eq!(model.to_char_position(utf16), position);
        }
    }

    #[test]
    fn test_to_char_position_clamps_a_column_past_the_end_of_the_line() {
        let model = TextModel::from_text("abc");
        assert_eq!(
            model.to_char_position(LspPosition { line: 0, character: 99 }),
            LspPosition { line: 0, character: 3 }
        );
    }

    #[test]
    fn test_to_char_position_on_an_unknown_line_yields_column_zero() {
        let model = TextModel::from_text("abc");
        assert_eq!(
            model.to_char_position(LspPosition { line: 9, character: 2 }),
            LspPosition { line: 9, character: 0 }
        );
    }

    // -------------------------------------------------------------------------

    fn change(
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
        text: &str,
    ) -> LspTextChange {
        LspTextChange {
            range: LspRange {
                start: LspPosition { line: start_line, character: start_char },
                end: LspPosition { line: end_line, character: end_char },
            },
            text: text.to_string(),
        }
    }

    #[test]
    fn test_text_model_apply_change_in_range_succeeds() {
        let mut model = TextModel::from_text("hello\nworld");
        let applied = model.apply_change(&change(0, 0, 0, 5, "goodbye"));
        assert!(applied);
        assert_eq!(
            model.lines,
            vec!["goodbye".to_string(), "world".to_string()]
        );
    }

    #[test]
    fn test_text_model_apply_change_out_of_range_line_fails_without_mutating() {
        let mut model = TextModel::from_text("hello\nworld");
        let original = model.lines.clone();

        // Line 5 does not exist in a 2-line document.
        let applied = model.apply_change(&change(5, 0, 5, 0, "x"));

        assert!(!applied);
        assert_eq!(model.lines, original);
    }

    // -------------------------------------------------------------------------
    // apply_changes_to_document
    // -------------------------------------------------------------------------

    #[test]
    #[allow(clippy::panic)]
    fn test_apply_changes_to_document_converts_every_change() {
        let mut state =
            DocumentState { text: TextModel::from_text("hello\nworld") };

        let changes =
            vec![change(0, 0, 0, 5, "hi"), change(1, 0, 1, 5, "earth")];
        let Some(out) = apply_changes_to_document(&mut state, &changes) else {
            panic!("in-range changes must convert");
        };

        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["text"], "hi");
        assert_eq!(out[1]["text"], "earth");
        assert_eq!(
            state.text.lines,
            vec!["hi".to_string(), "earth".to_string()]
        );
    }

    #[test]
    fn test_apply_changes_to_document_stops_at_first_desync() {
        let mut state = DocumentState { text: TextModel::from_text("hello") };

        // The first change is valid and does get applied to the mirror; the
        // second references a line that doesn't exist. The whole batch must
        // report `None` rather than a partial result, since every change
        // after the desync point is computed against a mirror state that no
        // longer reflects reality.
        let changes =
            vec![change(0, 0, 0, 5, "hi"), change(9, 0, 9, 0, "unreachable")];
        let out = apply_changes_to_document(&mut state, &changes);

        assert!(out.is_none());
        // The first change was still applied before the desync was found —
        // documenting why the caller must discard the whole `DocumentState`
        // rather than trying to salvage it.
        assert_eq!(state.text.lines, vec!["hi".to_string()]);
    }
}
