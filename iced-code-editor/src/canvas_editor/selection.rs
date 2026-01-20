//! Text selection logic.

use super::CodeEditor;

impl CodeEditor {
    /// Clears the current selection.
    pub(crate) fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
        self.cache.clear();
    }

    /// Returns the selected text range in normalized order (start before end).
    pub(crate) fn get_selection_range(
        &self,
    ) -> Option<((usize, usize), (usize, usize))> {
        if let (Some(start), Some(end)) =
            (self.selection_start, self.selection_end)
        {
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
            // Notes:
            // - Column indices (start.1 / end.1) are character indices (Unicode scalar count).
            // - Rust strings are UTF-8 bytes; slicing by character indices can panic.
            // - Convert character indices to byte indices via char_indices() before slicing.
            // Convert UTF-8 character indices to byte indices for safe slicing
            let start_byte = line
                .char_indices()
                .nth(start.1)
                .map_or(line.len(), |(idx, _)| idx);
            let end_byte = line
                .char_indices()
                .nth(end.1)
                .map_or(line.len(), |(idx, _)| idx);
            result.push_str(&line[start_byte..end_byte]);
        } else {
            // Multi-line selection
            // First line
            let first_line = self.buffer.line(start.0);
            // First line: convert the starting character index to a byte index and slice safely
            let start_byte = first_line
                .char_indices()
                .nth(start.1)
                .map_or(first_line.len(), |(idx, _)| idx);
            result.push_str(&first_line[start_byte..]);
            result.push('\n');

            // Middle lines
            for line_idx in (start.0 + 1)..end.0 {
                result.push_str(self.buffer.line(line_idx));
                result.push('\n');
            }

            // Last line
            let last_line = self.buffer.line(end.0);
            // Last line: convert the ending character index to a byte index and slice safely
            let end_byte = last_line
                .char_indices()
                .nth(end.1)
                .map_or(last_line.len(), |(idx, _)| idx);
            result.push_str(&last_line[..end_byte]);
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selection_single_line() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        let text = editor.get_selected_text();
        assert_eq!(text, Some("hello".to_string()));
    }

    #[test]
    fn test_selection_multiline() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        editor.selection_start = Some((0, 2)); // "ne1"
        editor.selection_end = Some((2, 3)); // to "lin"

        let text = editor.get_selected_text();
        assert_eq!(text, Some("ne1\nline2\nlin".to_string()));
    }

    #[test]
    fn test_selection_range_normalization() {
        let mut editor = CodeEditor::new("hello world", "py");
        // Set selection in reverse order (end before start)
        editor.selection_start = Some((0, 5));
        editor.selection_end = Some((0, 0));

        let range = editor.get_selection_range();
        // Should normalize to (0,0) -> (0,5)
        assert_eq!(range, Some(((0, 0), (0, 5))));
    }

    #[test]
    fn test_clear_selection() {
        let mut editor = CodeEditor::new("hello world", "py");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, 5));

        editor.clear_selection();
        assert_eq!(editor.selection_start, None);
        assert_eq!(editor.selection_end, None);
    }
}
