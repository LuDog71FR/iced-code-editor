//! Hidden re-exports for the benchmark harness in `benches/`.
//!
//! This module is compiled only with the `bench` feature and is **not** part
//! of the public API. It exposes internal hot-path functions so the
//! `criterion` benchmarks (which run as a separate crate) can measure them.
pub use super::folding::compute_foldable_regions;
pub use super::render::text::highlight_line_spans;
pub use super::render::wrapping::WrappingCalculator;
pub use super::search::find_matches;
pub use crate::buffer::TextBuffer;

/// Stateful harness for measuring the normal localized typing path.
pub struct IncrementalEditBenchmark {
    editor: super::CodeEditor,
}

impl IncrementalEditBenchmark {
    /// Creates an editor, primes its visual-line cache, and places the
    /// cursor at `line`/`column`.
    pub fn new(content: &str, line: usize, column: usize) -> Self {
        let mut editor =
            super::CodeEditor::new(content, "rs").with_wrap_column(Some(80));
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (line, column);
        let _ = editor.visual_lines_cached(800.0);
        Self { editor }
    }

    /// Inserts and removes one character, leaving content size stable for
    /// repeated Criterion iterations.
    pub fn insert_and_backspace(&mut self) -> u64 {
        let _ = self.editor.update(&super::Message::CharacterInput('x'));
        let _ = self.editor.update(&super::Message::Backspace);
        if self.editor.is_grouping {
            self.editor.history.end_group();
            self.editor.is_grouping = false;
        }
        self.editor.buffer_revision
    }
}

struct NoopLspClient;

impl super::lsp::LspClient for NoopLspClient {}

/// Stateful harness for the incremental LSP synchronization path.
pub struct IncrementalLspEditBenchmark {
    editor: super::CodeEditor,
}

impl IncrementalLspEditBenchmark {
    /// Creates and primes a focused editor with a no-op LSP client.
    pub fn new(content: &str, line: usize, column: usize) -> Self {
        let mut editor =
            super::CodeEditor::new(content, "rs").with_wrap_column(Some(80));
        editor.attach_lsp(
            Box::new(NoopLspClient),
            super::lsp::LspDocument::new("file:///benchmark.rs", "rust"),
        );
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (line, column);
        let _ = editor.visual_lines_cached(800.0);
        Self { editor }
    }

    /// Inserts and removes one character while sending incremental LSP
    /// changes for both edits.
    pub fn insert_and_backspace(&mut self) -> u64 {
        let _ = self.editor.update(&super::Message::CharacterInput('x'));
        let _ = self.editor.update(&super::Message::Backspace);
        if self.editor.is_grouping {
            self.editor.history.end_group();
            self.editor.is_grouping = false;
        }
        self.editor.buffer_revision
    }
}

/// Stateful harness for typing with wrapping disabled.
pub struct IncrementalNoWrapEditBenchmark {
    editor: super::CodeEditor,
}

impl IncrementalNoWrapEditBenchmark {
    /// Creates an editor and primes both layout and horizontal-width caches.
    pub fn new(content: &str, line: usize, column: usize) -> Self {
        let mut editor = super::CodeEditor::new(content, "rs");
        editor.set_wrap_enabled(false);
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (line, column);
        let _ = editor.visual_lines_cached(800.0);
        let _ = editor.max_content_width();
        Self { editor }
    }

    /// Inserts and removes one character without triggering a whole-file
    /// maximum-width scan.
    pub fn insert_and_backspace(&mut self) -> u64 {
        let _ = self.editor.update(&super::Message::CharacterInput('x'));
        let _ = self.editor.update(&super::Message::Backspace);
        if self.editor.is_grouping {
            self.editor.history.end_group();
            self.editor.is_grouping = false;
        }
        self.editor.buffer_revision
    }
}

/// Stateful harness for typing while a large-file search is open.
pub struct IncrementalSearchEditBenchmark {
    editor: super::CodeEditor,
}

impl IncrementalSearchEditBenchmark {
    /// Creates an editor with populated search results and a warm layout.
    pub fn new(content: &str, query: &str, line: usize, column: usize) -> Self {
        let mut editor =
            super::CodeEditor::new(content, "rs").with_wrap_column(Some(80));
        editor.search_state.open_search();
        editor.search_state.set_query(query.to_owned(), &editor.buffer);
        editor.request_focus();
        editor.has_canvas_focus = true;
        editor.focus_locked = false;
        editor.cursors.primary_mut().position = (line, column);
        let _ = editor.visual_lines_cached(800.0);
        Self { editor }
    }

    /// Inserts and removes one character while maintaining search matches.
    pub fn insert_and_backspace(&mut self) -> u64 {
        let _ = self.editor.update(&super::Message::CharacterInput('x'));
        let _ = self.editor.update(&super::Message::Backspace);
        if self.editor.is_grouping {
            self.editor.history.end_group();
            self.editor.is_grouping = false;
        }
        self.editor.buffer_revision
    }
}

/// Measures the incremental wrapping path without exposing internal visual
/// line types as public editor API.
pub fn calculate_visual_line_range_len(
    calculator: &WrappingCalculator,
    buffer: &TextBuffer,
    viewport_width: f32,
    gutter_width: f32,
    start_line: usize,
    end_line: usize,
) -> usize {
    calculator
        .calculate_visual_lines_range(
            buffer,
            viewport_width,
            gutter_width,
            &std::collections::HashSet::new(),
            start_line..end_line,
        )
        .len()
}
