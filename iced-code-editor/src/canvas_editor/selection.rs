//! Text selection logic.

use super::CanvasEditor;

impl CanvasEditor {
    /// Clears the current selection.
    pub(crate) fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
        self.cache.clear();
    }

    /// Returns the selected text range in normalized order (start before end).
    pub(crate) fn get_selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
        if let (Some(start), Some(end)) = (self.selection_start, self.selection_end) {
            // Normalize: ensure start comes before end
            if start.0 < end.0 || (start.0 == end.0 && start.1 < end.1) {
                Some((start, end))
            } else {
                Some((end, start))
            }
        } else {
            None
        }
    }

    /// Returns the selected text as a string.
    pub(crate) fn get_selected_text(&self) -> Option<String> {
        let (start, end) = self.get_selection_range()?;

        if start == end {
            return None; // No selection
        }

        let mut result = String::new();

        if start.0 == end.0 {
            // Single line selection
            let line = self.buffer.line(start.0);
            result.push_str(&line[start.1..end.1]);
        } else {
            // Multi-line selection
            // First line
            let first_line = self.buffer.line(start.0);
            result.push_str(&first_line[start.1..]);
            result.push('\n');

            // Middle lines
            for line_idx in (start.0 + 1)..end.0 {
                result.push_str(self.buffer.line(line_idx));
                result.push('\n');
            }

            // Last line
            let last_line = self.buffer.line(end.0);
            result.push_str(&last_line[..end.1]);
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_single_line() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let text = editor.get_selected_text();
        assert_eq!(text, Some("hello".to_string()));
    }

    #[test]
    fn test_selection_multiline() {
        let mut editor = CanvasEditor::new("line1\nline2\nline3", "py");
        editor.selection_start = Some((0, 2)); // "ne1"
        editor.selection_end = Some((2, 3)); // to "lin"

        let text = editor.get_selected_text();
        assert_eq!(text, Some("ne1\nline2\nlin".to_string()));
    }

    #[test]
    fn test_selection_range_normalization() {
        let mut editor = CanvasEditor::new("hello world", "py");
        // Set selection in reverse order (end before start)
        editor.selection_start = Some((0, 5));
        editor.selection_end = Some((0, 0));

        let range = editor.get_selection_range();
        // Should normalize to (0,0) -> (0,5)
        assert_eq!(range, Some(((0, 0), (0, 5))));
    }

    #[test]
    fn test_clear_selection() {
        let mut editor = CanvasEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        editor.clear_selection();
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }
}
