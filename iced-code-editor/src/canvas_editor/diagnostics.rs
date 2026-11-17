//! Diagnostic ranges and their placement on visual lines.

use crate::canvas_editor::CodeEditor;
use crate::canvas_editor::lsp::Diagnostic;
use crate::canvas_editor::render::wrapping::VisualLine;

impl CodeEditor {
    /// Replaces the diagnostic ranges underlined by the editor.
    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        if self.diagnostics == diagnostics {
            return;
        }
        self.diagnostics = diagnostics;
        self.overlay_cache.clear();
    }

    /// The diagnostic ranges currently underlined by the editor.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
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
        Diagnostic {
            range: LspRange {
                start: LspPosition { line: start.0, character: start.1 },
                end: LspPosition { line: end.0, character: end.1 },
            },
            severity: DiagnosticSeverity::Error,
        }
    }

    #[test]
    fn diagnostics_are_replaced_and_cleared() {
        let mut editor = CodeEditor::new("let answer = 42;\n", "rs");
        let diagnostic = diagnostic((0, 4), (0, 10));
        editor.set_diagnostics(vec![diagnostic]);
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
