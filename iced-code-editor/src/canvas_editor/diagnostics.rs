//! Diagnostic ranges and their placement on visual lines.

use crate::canvas_editor::CodeEditor;
use crate::canvas_editor::lsp::{Diagnostic, LspPosition};
use crate::canvas_editor::render::wrapping::VisualLine;

impl CodeEditor {
    /// Replaces the diagnostic ranges underlined by the editor.
    ///
    /// Columns count characters, not UTF-16 code units, and the end is
    /// exclusive. The ranges are not moved when the text is edited: pass a new
    /// list whenever the diagnostics change. An empty list clears them.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{
    ///     CodeEditor, Diagnostic, DiagnosticSeverity, LspPosition, LspRange,
    /// };
    ///
    /// let mut editor = CodeEditor::new("let answer = 42;", "rs");
    /// let diagnostic = Diagnostic::new(
    ///     LspRange {
    ///         start: LspPosition { line: 0, character: 4 },
    ///         end: LspPosition { line: 0, character: 10 },
    ///     },
    ///     DiagnosticSeverity::Warning,
    ///     "unused variable",
    /// );
    /// editor.set_diagnostics(vec![diagnostic.clone()]);
    /// assert_eq!(editor.diagnostics(), &[diagnostic]);
    /// ```
    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        if self.diagnostics == diagnostics {
            return;
        }
        self.diagnostics = diagnostics;
        self.overlay_cache.clear();
    }

    /// The diagnostic ranges currently underlined by the editor.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!(editor.diagnostics().is_empty());
    /// ```
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// The diagnostics covering `position`, in the order they were set.
    ///
    /// Always empty while diagnostics are disabled
    /// ([`CodeEditor::set_diagnostics_enabled`]).
    ///
    /// `position` is in logical lines and character columns, as returned by
    /// [`CodeEditor::lsp_position_at_point`]. Hosts use it to show the
    /// messages of the diagnostics under the pointer.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{
    ///     CodeEditor, Diagnostic, DiagnosticSeverity, LspPosition, LspRange,
    /// };
    ///
    /// let mut editor = CodeEditor::new("let answer = 42;", "rs");
    /// editor.set_diagnostics(vec![Diagnostic::new(
    ///     LspRange {
    ///         start: LspPosition { line: 0, character: 4 },
    ///         end: LspPosition { line: 0, character: 10 },
    ///     },
    ///     DiagnosticSeverity::Warning,
    ///     "unused variable",
    /// )]);
    ///
    /// let hovered = editor.diagnostics_at(LspPosition { line: 0, character: 6 });
    /// assert_eq!(hovered.len(), 1);
    /// assert_eq!(hovered[0].message, "unused variable");
    /// assert!(editor.diagnostics_at(LspPosition { line: 0, character: 0 }).is_empty());
    /// ```
    pub fn diagnostics_at(&self, position: LspPosition) -> Vec<&Diagnostic> {
        if !self.diagnostics_enabled {
            return Vec::new();
        }
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.contains(position))
            .collect()
    }
}

/// Intersects a diagnostic with one visual line, in logical character columns.
pub(super) fn diagnostic_segment(
    visual_line: &VisualLine,
    diagnostic: &Diagnostic,
) -> Option<(usize, usize)> {
    let start = diagnostic.range.start;
    let end = diagnostic.range.end;
    if (start.line, start.character) > (end.line, end.character) {
        return None;
    }

    if start == end {
        let col = start.character as usize;
        return (visual_line.logical_line == start.line as usize
            && visual_line.start_col <= col
            && (col < visual_line.end_col
                || (visual_line.start_col == visual_line.end_col
                    && col == visual_line.start_col)))
            .then_some((col, col + 1));
    }

    let line = visual_line.logical_line as u32;
    let start_col = if line == start.line {
        start.character as usize
    } else {
        visual_line.start_col
    };
    let end_col = if line == end.line {
        end.character as usize
    } else {
        visual_line.end_col
    };
    let start_col = start_col.max(visual_line.start_col);
    let end_col = end_col.min(visual_line.end_col);

    (line >= start.line && line <= end.line && start_col < end_col)
        .then_some((start_col, end_col))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::lsp::{
        DiagnosticSeverity, LspPosition, LspRange,
    };

    fn diagnostic(start: (u32, u32), end: (u32, u32)) -> Diagnostic {
        Diagnostic::new(
            LspRange {
                start: LspPosition { line: start.0, character: start.1 },
                end: LspPosition { line: end.0, character: end.1 },
            },
            DiagnosticSeverity::Error,
            "error",
        )
    }

    fn at(line: u32, character: u32) -> LspPosition {
        LspPosition { line, character }
    }

    #[test]
    fn disabling_diagnostics_hides_them_but_keeps_the_list() {
        let mut editor = CodeEditor::new("let answer = 42;\n", "rs");
        let error = diagnostic((0, 4), (0, 10));
        editor.set_diagnostics_enabled(false);
        // Set while disabled: stored, not reported.
        editor.set_diagnostics(vec![error.clone()]);
        assert!(editor.diagnostics_at(at(0, 5)).is_empty());
        assert_eq!(editor.diagnostics(), std::slice::from_ref(&error));

        editor.set_diagnostics_enabled(true);
        assert_eq!(editor.diagnostics_at(at(0, 5)), vec![&error]);
    }

    #[test]
    fn diagnostic_contains_spans_lines_with_exclusive_end() {
        let multi = diagnostic((0, 8), (2, 3));
        assert!(!multi.contains(at(0, 7)));
        assert!(multi.contains(at(0, 8)));
        assert!(multi.contains(at(1, 0)));
        assert!(multi.contains(at(2, 2)));
        assert!(!multi.contains(at(2, 3)));
    }

    #[test]
    fn diagnostic_contains_empty_and_reversed_ranges() {
        let empty = diagnostic((1, 4), (1, 4));
        assert!(empty.contains(at(1, 4)));
        assert!(!empty.contains(at(1, 5)));

        let reversed = diagnostic((1, 6), (1, 2));
        assert!(!reversed.contains(at(1, 4)));
    }

    #[test]
    fn diagnostics_at_returns_every_covering_diagnostic_in_order() {
        let mut editor = CodeEditor::new("let answer = 42;\n", "rs");
        let outer = diagnostic((0, 0), (0, 16));
        let inner = diagnostic((0, 4), (0, 10));
        let elsewhere = diagnostic((0, 13), (0, 15));
        editor.set_diagnostics(vec![
            outer.clone(),
            inner.clone(),
            elsewhere.clone(),
        ]);

        assert_eq!(editor.diagnostics_at(at(0, 5)), vec![&outer, &inner]);
        assert_eq!(editor.diagnostics_at(at(0, 14)), vec![&outer, &elsewhere]);
        assert!(editor.diagnostics_at(at(1, 0)).is_empty());
    }

    #[test]
    fn diagnostics_are_replaced_and_cleared() {
        let mut editor = CodeEditor::new("let answer = 42;\n", "rs");
        let diagnostic = diagnostic((0, 4), (0, 10));
        editor.set_diagnostics(vec![diagnostic.clone()]);
        assert_eq!(editor.diagnostics(), &[diagnostic]);
        editor.set_diagnostics(Vec::new());
        assert!(editor.diagnostics().is_empty());
    }

    #[test]
    fn diagnostic_segments_follow_wrapping_and_multiple_lines() {
        let wrapped = VisualLine::new(0, 1, 10, 20);
        assert_eq!(
            diagnostic_segment(&wrapped, &diagnostic((0, 8), (0, 15))),
            Some((10, 15))
        );
        assert_eq!(
            diagnostic_segment(&wrapped, &diagnostic((0, 20), (0, 25))),
            None
        );

        let middle = VisualLine::new(1, 0, 0, 12);
        assert_eq!(
            diagnostic_segment(&middle, &diagnostic((0, 8), (2, 3))),
            Some((0, 12))
        );

        let empty = VisualLine::new(0, 0, 0, 10);
        assert_eq!(
            diagnostic_segment(&empty, &diagnostic((0, 4), (0, 4))),
            Some((4, 5))
        );
        let empty_line = VisualLine::new(2, 0, 0, 0);
        assert_eq!(
            diagnostic_segment(&empty_line, &diagnostic((2, 0), (2, 0))),
            Some((0, 1))
        );
    }
}
