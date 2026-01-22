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
            // Validate that character indices are within the valid range before attempting to slice.
            // This prevents potential issues if start.1 or end.1 exceed the actual character count.
            if let Some((start_byte, _)) = line.char_indices().nth(start.1) {
                let end_byte = line
                    .char_indices()
                    .nth(end.1)
                    .map_or(line.len(), |(idx, _)| idx);
                result.push_str(&line[start_byte..end_byte]);
            }
        } else {
            // Multi-line selection
            // First line
            let first_line = self.buffer.line(start.0);
            // First line: convert the starting character index to a byte index and slice safely
            // Validate that character indices are within the valid range before attempting to slice.
            // This prevents potential issues if start.1 exceed the actual character count.
            if let Some((start_byte, _)) =
                first_line.char_indices().nth(start.1)
            {
                result.push_str(&first_line[start_byte..]);
                result.push('\n');
            }

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

    #[test]
    fn test_selection_out_of_bounds() {
        let mut editor = CodeEditor::new("hello", "py");
        // Start out of bounds (column 10)
        editor.selection_start = Some((0, 10));
        editor.selection_end = Some((0, 15));

        let text = editor.get_selected_text();
        // With the fix, start is out of bounds, so we get empty string.
        assert_eq!(text, Some("".to_string()));
    }

    #[test]
    fn test_selection_multiline_out_of_bounds() {
        let mut editor = CodeEditor::new("line1\nline2\nline3", "py");
        // Start out of bounds on first line
        editor.selection_start = Some((0, 10));
        // End normal on last line
        editor.selection_end = Some((2, 3));

        let text = editor.get_selected_text();
        // First line is skipped because start is out of bounds.
        // Middle line (line2) is included.
        // Last line is included up to index 3 ("lin").
        // Result: "\nline2\nlin" (The newline from first line is pushed if start is valid?
        // Let's check logic: if start is invalid, nothing from first line is pushed, including newline)
        // Actually, looking at the code:
        // if let Some((start_byte, _)) = first_line.char_indices().nth(start.1) { ... result.push('\n'); }
        // So if start is out of bounds, NO newline is added for the first line.
        // Wait, if first line is skipped entirely, we just get middle lines and last line.
        assert_eq!(text, Some("line2\nlin".to_string()));

        // Now test end out of bounds
        editor.selection_start = Some((0, 2));
        editor.selection_end = Some((2, 10)); // End out of bounds on last line
        let text = editor.get_selected_text();
        // "ne1\n" + "line2\n" + "line3" (entire last line)
        assert_eq!(text, Some("ne1\nline2\nline3".to_string()));
    }

    #[test]
    fn test_selection_unicode() {
        // "你好" (hello in Chinese) - 2 chars, but 6 bytes
        // "世界" (world in Chinese)
        let mut editor = CodeEditor::new("你好\n世界", "txt");

        // Select '好' (index 1 on line 0) to '世' (index 1 on line 1, exclusive? No, end is exclusive usually)
        // Wait, end index is character index.
        // Line 0: 你(0) 好(1)
        // Line 1: 世(0) 界(1)

        // Select from (0, 1) -> '好' starts at char index 1
        // To (1, 1) -> '世' is at char index 0. End at 1 means include char 0.
        editor.selection_start = Some((0, 1));
        editor.selection_end = Some((1, 1));

        let text = editor.get_selected_text();
        // Should be "好\n世"
        assert_eq!(text, Some("好\n世".to_string()));
    }

    #[test]
    fn test_selection_with_empty_lines() {
        let mut editor = CodeEditor::new("line1\n\nline3", "txt");
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((2, 5));

        let text = editor.get_selected_text();
        assert_eq!(text, Some("line1\n\nline3".to_string()));
    }

    #[test]
    fn test_selection_emoji() {
        // "a😀b"
        // 'a' (1 byte), '😀' (4 bytes), 'b' (1 byte)
        let mut editor = CodeEditor::new("a😀b", "txt");

        // Select '😀'
        // 'a' is at index 0
        // '😀' is at index 1
        // 'b' is at index 2
        editor.selection_start = Some((0, 1));
        editor.selection_end = Some((0, 2));

        let text = editor.get_selected_text();
        assert_eq!(text, Some("😀".to_string()));
    }

    #[test]
    fn test_selection_complex_emoji() {
        // 👨‍👩‍👧‍👦 is a ZWJ sequence
        let complex_emoji = "👨‍👩‍👧‍👦";
        let mut editor = CodeEditor::new(complex_emoji, "txt");

        // Count chars
        let char_count = complex_emoji.chars().count();

        // Select the whole thing
        editor.selection_start = Some((0, 0));
        editor.selection_end = Some((0, char_count));

        let text = editor.get_selected_text();
        assert_eq!(text, Some(complex_emoji.to_string()));

        // Select partial (just the first component)
        // This confirms we don't crash, even if it splits the grapheme.
        // It should return the first scalar value (Man).
        if char_count > 1 {
            editor.selection_start = Some((0, 0));
            editor.selection_end = Some((0, 1));
            let text = editor.get_selected_text();
            let first_char = complex_emoji.chars().next().unwrap().to_string();
            assert_eq!(text, Some(first_char));
        }
    }
}
