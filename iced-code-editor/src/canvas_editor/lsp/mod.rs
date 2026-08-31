//! Minimal LSP types and helpers used by the editor.

pub(crate) mod edits;
pub(crate) mod sync;

use crate::canvas_editor::IndentStyle;
use crate::canvas_editor::metrics::TAB_WIDTH;

#[cfg(all(feature = "lsp-process", not(target_arch = "wasm32")))]
pub mod process;

/// A zero-based position in an LSP document.
///
/// Both fields are zero-based, unlike the one-based line numbers shown in the
/// editor's gutter.
///
/// # Example
///
/// ```
/// use iced_code_editor::LspPosition;
///
/// // The gutter's "line 3, column 1" is this position.
/// let position = LspPosition { line: 2, character: 0 };
/// assert_eq!(position.line, 2);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspPosition {
    /// Zero-based line index.
    pub line: u32,
    /// Zero-based character index on the line.
    pub character: u32,
}

/// Metadata describing the currently edited document.
///
/// The editor stamps `version` itself: [`CodeEditor::attach_lsp`] sets it to 1
/// on open, and each flushed batch of changes increments it.
///
/// # Example
///
/// ```
/// use iced_code_editor::LspDocument;
///
/// let document = LspDocument::new("file:///tmp/main.rs", "rust");
/// assert_eq!(document.uri, "file:///tmp/main.rs");
/// assert_eq!(document.language_id, "rust");
/// ```
///
/// [`CodeEditor::attach_lsp`]: crate::CodeEditor::attach_lsp
#[derive(Debug, Clone)]
pub struct LspDocument {
    /// Document URI.
    pub uri: String,
    /// Language identifier for syntax services.
    pub language_id: String,
    /// Version number used for LSP change notifications.
    pub version: i32,
}

impl LspDocument {
    /// Creates a new LSP document descriptor with version set to 0.
    ///
    /// # Arguments
    ///
    /// * `uri` - The document URI, e.g. `file:///path/to/main.rs`
    /// * `language_id` - The LSP language identifier, e.g. `rust`
    ///
    /// # Returns
    ///
    /// A descriptor at version 0; the editor stamps version 1 on open
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::LspDocument;
    ///
    /// let document = LspDocument::new("file:///tmp/main.rs", "rust");
    /// assert_eq!(document.version, 0);
    /// ```
    pub fn new(uri: impl Into<String>, language_id: impl Into<String>) -> Self {
        Self { uri: uri.into(), language_id: language_id.into(), version: 0 }
    }
}

/// A text range in an LSP document.
///
/// `start` is inclusive and `end` is exclusive, so an empty range (a pure
/// insertion point) has `start == end`.
///
/// # Example
///
/// ```
/// use iced_code_editor::{LspPosition, LspRange};
///
/// // The first four characters of the first line.
/// let range = LspRange {
///     start: LspPosition { line: 0, character: 0 },
///     end: LspPosition { line: 0, character: 4 },
/// };
/// assert_eq!(range.end.character - range.start.character, 4);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct LspRange {
    /// Range start (inclusive).
    pub start: LspPosition,
    /// Range end (exclusive).
    pub end: LspPosition,
}

/// A text change described by a range replacement.
///
/// An insertion is an empty `range` with non-empty `text`; a deletion is a
/// non-empty `range` with empty `text`.
///
/// # Example
///
/// ```
/// use iced_code_editor::{LspPosition, LspRange, LspTextChange};
///
/// // Insert "hi" at the start of the document: an empty range.
/// let insertion = LspTextChange {
///     range: LspRange {
///         start: LspPosition { line: 0, character: 0 },
///         end: LspPosition { line: 0, character: 0 },
///     },
///     text: "hi".to_string(),
/// };
/// assert_eq!(insertion.range.start, insertion.range.end);
/// ```
#[derive(Debug, Clone)]
pub struct LspTextChange {
    /// Range replaced by the change.
    pub range: LspRange,
    /// Inserted text.
    pub text: String,
}

/// Formatting options sent with a document-formatting request.
///
/// Mirrors the two fields of LSP's `FormattingOptions` that every server
/// requires. Build one from the editor's own indentation setting with the
/// `From<IndentStyle>` implementation rather than hardcoding values, so the
/// server formats the way the editor indents.
///
/// # Example
///
/// ```
/// use iced_code_editor::{IndentStyle, LspFormattingOptions};
///
/// let options = LspFormattingOptions::from(IndentStyle::Spaces(2));
/// assert_eq!(options.tab_size, 2);
/// assert!(options.insert_spaces);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LspFormattingOptions {
    /// Number of columns a single indentation level spans.
    pub tab_size: u32,
    /// `true` to indent with spaces, `false` to indent with tab characters.
    pub insert_spaces: bool,
}

impl From<IndentStyle> for LspFormattingOptions {
    /// Derives formatting options from the editor's indentation style.
    ///
    /// [`IndentStyle::Tab`] reports the editor's own rendered tab width, which
    /// is what the server needs to lay out alignment inside a tab-indented
    /// file. A zero-width spaces style — which no built-in preset produces —
    /// is raised to 1, since `tabSize: 0` is not a value any server can act on.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{IndentStyle, LspFormattingOptions};
    ///
    /// let options = LspFormattingOptions::from(IndentStyle::Tab);
    /// assert!(!options.insert_spaces);
    /// ```
    fn from(style: IndentStyle) -> Self {
        match style {
            IndentStyle::Spaces(width) => {
                Self { tab_size: u32::from(width).max(1), insert_spaces: true }
            }
            IndentStyle::Tab => {
                Self { tab_size: TAB_WIDTH as u32, insert_spaces: false }
            }
        }
    }
}

/// LSP client hooks invoked by the editor.
///
/// Every method has a no-op default, so an implementation only overrides the
/// notifications and requests it actually cares about. Attach one with
/// [`CodeEditor::attach_lsp`]; the editor never blocks on a response, so
/// request results come back through whatever channel the implementation
/// chooses.
///
/// # Example
///
/// ```
/// use std::cell::RefCell;
/// use std::rc::Rc;
///
/// use iced_code_editor::{CodeEditor, LspClient, LspDocument, LspTextChange};
///
/// /// A client that only records how many change batches it received.
/// struct CountingClient(Rc<RefCell<usize>>);
///
/// impl LspClient for CountingClient {
///     fn did_change(&mut self, _document: &LspDocument, changes: &[LspTextChange]) {
///         *self.0.borrow_mut() += changes.len();
///     }
/// }
///
/// let changes = Rc::new(RefCell::new(0));
/// let mut editor = CodeEditor::new("fn main() {}", "rs");
/// editor.attach_lsp(
///     Box::new(CountingClient(Rc::clone(&changes))),
///     LspDocument::new("file:///tmp/main.rs", "rust"),
/// );
///
/// // `did_open` fired, but no change has been flushed yet.
/// assert_eq!(*changes.borrow(), 0);
/// ```
///
/// [`CodeEditor::attach_lsp`]: crate::CodeEditor::attach_lsp
pub trait LspClient {
    /// Notifies the client that a document was opened.
    fn did_open(&mut self, _document: &LspDocument, _text: &str) {}
    /// Notifies the client that the document changed.
    fn did_change(
        &mut self,
        _document: &LspDocument,
        _changes: &[LspTextChange],
    ) {
    }
    /// Notifies the client that the document was saved.
    fn did_save(&mut self, _document: &LspDocument, _text: &str) {}
    /// Notifies the client that the document was closed.
    fn did_close(&mut self, _document: &LspDocument) {}
    /// Requests hover information at the given position.
    fn request_hover(
        &mut self,
        _document: &LspDocument,
        _position: LspPosition,
    ) {
    }
    /// Requests completion items at the given position.
    fn request_completion(
        &mut self,
        _document: &LspDocument,
        _position: LspPosition,
    ) {
    }
    /// Requests the definition location(s) for the symbol at the given position.
    ///
    /// This method is called when the user triggers a "Go to Definition" action
    /// (e.g., via Ctrl+Click or a context menu). The client implementation should
    /// send a `textDocument/definition` request to the LSP server.
    fn request_definition(
        &mut self,
        _document: &LspDocument,
        _position: LspPosition,
    ) {
    }
    /// Requests formatting edits for the whole document.
    ///
    /// This method is called when the user triggers a "Format Document"
    /// action, or when the host formats before saving. The client
    /// implementation should send a `textDocument/formatting` request to the
    /// LSP server; the resulting edits come back through whatever channel the
    /// implementation uses, and the host applies them with
    /// [`CodeEditor::apply_lsp_text_edits`].
    ///
    /// [`CodeEditor::apply_lsp_text_edits`]: crate::CodeEditor::apply_lsp_text_edits
    fn request_formatting(
        &mut self,
        _document: &LspDocument,
        _options: LspFormattingOptions,
    ) {
    }
}

/// Computes a minimal text change between two snapshots.
///
/// The change is narrowed by trimming the common prefix and suffix, so a
/// one-character edit in a large document produces a one-character change
/// rather than a whole-document replacement. This is what lets the editor send
/// incremental `didChange` notifications instead of resending the buffer.
///
/// # Arguments
///
/// * `old` - The previous document contents
/// * `new` - The current document contents
///
/// # Returns
///
/// `Some(change)` describing the edit, or `None` when the snapshots are identical
///
/// # Example
///
/// ```
/// use iced_code_editor::compute_text_change;
///
/// // Identical snapshots produce no change at all.
/// assert!(compute_text_change("fn main() {}", "fn main() {}").is_none());
///
/// // Only the differing region is reported, not the whole line.
/// let change = compute_text_change("let x = 1;", "let x = 42;")
///     .expect("the snapshots differ");
/// assert_eq!(change.text, "42");
/// assert_eq!(change.range.start.character, 8);
/// assert_eq!(change.range.end.character, 9);
/// ```
pub fn compute_text_change(old: &str, new: &str) -> Option<LspTextChange> {
    if old == new {
        return None;
    }

    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let old_len = old_chars.len();
    let new_len = new_chars.len();

    let mut prefix = 0;
    while prefix < old_len
        && prefix < new_len
        && old_chars[prefix] == new_chars[prefix]
    {
        prefix += 1;
    }

    let mut suffix = 0;
    while suffix < old_len.saturating_sub(prefix)
        && suffix < new_len.saturating_sub(prefix)
        && old_chars[old_len - 1 - suffix] == new_chars[new_len - 1 - suffix]
    {
        suffix += 1;
    }

    let removed_len = old_len.saturating_sub(prefix + suffix);
    let inserted: String =
        new_chars[prefix..new_len.saturating_sub(suffix)].iter().collect();

    let start = position_for_char_index(old, prefix);
    let end = position_for_char_index(old, prefix + removed_len);

    Some(LspTextChange { range: LspRange { start, end }, text: inserted })
}

/// Converts a character index into a line/character position.
fn position_for_char_index(text: &str, target_index: usize) -> LspPosition {
    let mut line: u32 = 0;
    let mut character: u32 = 0;
    for (index, ch) in text.chars().enumerate() {
        if index == target_index {
            return LspPosition { line, character };
        }
        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        } else {
            character = character.saturating_add(1);
        }
    }

    LspPosition { line, character }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_text_change_none_when_equal() {
        let change = compute_text_change("abc", "abc");
        assert!(change.is_none());
    }

    #[test]
    fn test_compute_text_change_insertion() {
        let change = compute_text_change("abc", "abXc");
        assert!(change.is_some());
        if let Some(change) = change {
            assert_eq!(change.text, "X");
            assert_eq!(
                change.range.start,
                LspPosition { line: 0, character: 2 }
            );
            assert_eq!(change.range.end, LspPosition { line: 0, character: 2 });
        }
    }

    #[test]
    fn test_compute_text_change_deletion_across_lines() {
        let change = compute_text_change("a\nbc", "a\nc");
        assert!(change.is_some());
        if let Some(change) = change {
            assert_eq!(change.text, "");
            assert_eq!(
                change.range.start,
                LspPosition { line: 1, character: 0 }
            );
            assert_eq!(change.range.end, LspPosition { line: 1, character: 1 });
        }
    }

    #[test]
    fn test_position_for_char_index_end_of_text() {
        let pos = position_for_char_index("a\nb", 3);
        assert_eq!(pos, LspPosition { line: 1, character: 1 });
    }

    #[test]
    fn test_formatting_options_from_spaces_indent_style() {
        let options = LspFormattingOptions::from(IndentStyle::Spaces(2));
        assert_eq!(
            options,
            LspFormattingOptions { tab_size: 2, insert_spaces: true }
        );
    }

    #[test]
    fn test_formatting_options_from_tab_indent_style() {
        let options = LspFormattingOptions::from(IndentStyle::Tab);
        assert_eq!(
            options,
            LspFormattingOptions {
                tab_size: TAB_WIDTH as u32,
                insert_spaces: false,
            }
        );
    }

    #[test]
    fn test_formatting_options_never_report_a_zero_tab_size() {
        // `tabSize: 0` is not a value a server can lay text out with.
        assert_eq!(
            LspFormattingOptions::from(IndentStyle::Spaces(0)).tab_size,
            1
        );
    }
}
