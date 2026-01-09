//! Line wrapping logic for the text editor.
//!
//! This module handles the calculation of visual lines from logical lines
//! when line wrapping is enabled. It supports both viewport-based wrapping
//! (dynamic) and fixed column wrapping.

use crate::text_buffer::TextBuffer;

use super::{CHAR_WIDTH, GUTTER_WIDTH};

/// Represents a visual line segment in the editor.
///
/// When line wrapping is enabled, a single logical line may be split into
/// multiple visual line segments that are displayed sequentially.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualLine {
    /// Index of the logical line in the text buffer
    pub logical_line: usize,
    /// Segment index (0 for first segment, 1+ for wrapped segments)
    pub segment_index: usize,
    /// Start column in the logical line (inclusive)
    pub start_col: usize,
    /// End column in the logical line (exclusive)
    pub end_col: usize,
}

impl VisualLine {
    /// Creates a new visual line segment.
    ///
    /// # Arguments
    ///
    /// * `logical_line` - Index of the logical line
    /// * `segment_index` - Index of the segment within the line
    /// * `start_col` - Start column (inclusive)
    /// * `end_col` - End column (exclusive)
    pub fn new(
        logical_line: usize,
        segment_index: usize,
        start_col: usize,
        end_col: usize,
    ) -> Self {
        Self { logical_line, segment_index, start_col, end_col }
    }

    /// Returns whether this is the first segment of the logical line.
    pub fn is_first_segment(&self) -> bool {
        self.segment_index == 0
    }

    /// Returns the length of this segment in characters.
    pub fn len(&self) -> usize {
        self.end_col - self.start_col
    }
}

/// Calculator for line wrapping operations.
///
/// Handles the conversion between logical lines (as stored in the text buffer)
/// and visual lines (as displayed on screen with wrapping applied).
pub struct WrappingCalculator {
    /// Whether wrapping is enabled
    wrap_enabled: bool,
    /// Fixed wrap column (None = wrap at viewport width)
    wrap_column: Option<usize>,
}

impl WrappingCalculator {
    /// Creates a new wrapping calculator.
    ///
    /// # Arguments
    ///
    /// * `wrap_enabled` - Whether line wrapping is enabled
    /// * `wrap_column` - Fixed wrap column, or None for viewport-based wrapping
    ///
    /// # Example
    ///
    /// ```ignore
    /// use iced_code_editor::canvas_editor::wrapping::WrappingCalculator;
    ///
    /// // Wrap at viewport width
    /// let calc = WrappingCalculator::new(true, None);
    ///
    /// // Wrap at 80 characters
    /// let calc = WrappingCalculator::new(true, Some(80));
    /// ```
    pub fn new(wrap_enabled: bool, wrap_column: Option<usize>) -> Self {
        Self { wrap_enabled, wrap_column }
    }

    /// Calculates all visual lines from the text buffer.
    ///
    /// # Arguments
    ///
    /// * `text_buffer` - The text buffer to wrap
    /// * `viewport_width` - Width of the viewport in pixels (used if wrap_column is None)
    ///
    /// # Returns
    ///
    /// A vector of visual line segments
    pub fn calculate_visual_lines(
        &self,
        text_buffer: &TextBuffer,
        viewport_width: f32,
    ) -> Vec<VisualLine> {
        if !self.wrap_enabled {
            // No wrapping: one visual line per logical line
            return (0..text_buffer.line_count())
                .map(|line| {
                    VisualLine::new(line, 0, 0, text_buffer.line_len(line))
                })
                .collect();
        }

        // Calculate wrap width in characters
        let wrap_width = self.calculate_wrap_width(viewport_width);

        let mut visual_lines = Vec::new();

        for logical_line in 0..text_buffer.line_count() {
            let line_len = text_buffer.line_len(logical_line);

            if line_len <= wrap_width {
                // Line fits in one segment
                visual_lines.push(VisualLine::new(
                    logical_line,
                    0,
                    0,
                    line_len,
                ));
            } else {
                // Split line into segments
                let mut start_col = 0;
                let mut segment_index = 0;

                while start_col < line_len {
                    let end_col = (start_col + wrap_width).min(line_len);
                    visual_lines.push(VisualLine::new(
                        logical_line,
                        segment_index,
                        start_col,
                        end_col,
                    ));
                    start_col = end_col;
                    segment_index += 1;
                }
            }
        }

        visual_lines
    }

    /// Converts a logical position to a visual line index.
    ///
    /// # Arguments
    ///
    /// * `visual_lines` - Pre-calculated visual lines
    /// * `line` - Logical line index
    /// * `col` - Column in the logical line
    ///
    /// # Returns
    ///
    /// The visual line index containing this position
    pub fn logical_to_visual(
        visual_lines: &[VisualLine],
        line: usize,
        col: usize,
    ) -> Option<usize> {
        visual_lines
            .iter()
            .position(|vl| {
                vl.logical_line == line
                    && col >= vl.start_col
                    && col < vl.end_col
            })
            .or_else(|| {
                // Handle cursor at end of line (col == end_col)
                visual_lines.iter().position(|vl| {
                    vl.logical_line == line && col == vl.end_col && {
                        // Check if this is the last segment for this line
                        visual_lines
                            .iter()
                            .filter(|v| v.logical_line == line)
                            .max_by_key(|v| v.segment_index)
                            .map(|v| v.segment_index == vl.segment_index)
                            .unwrap_or(false)
                    }
                })
            })
    }

    /// Calculates the wrap width in characters.
    ///
    /// # Arguments
    ///
    /// * `viewport_width` - Width of the viewport in pixels
    ///
    /// # Returns
    ///
    /// Maximum number of characters per line
    fn calculate_wrap_width(&self, viewport_width: f32) -> usize {
        match self.wrap_column {
            Some(col) => col,
            None => {
                // Calculate based on viewport width
                let available_width = viewport_width - GUTTER_WIDTH - 10.0; // Margins
                let chars = (available_width / CHAR_WIDTH) as usize;
                chars.max(20) // Minimum 20 characters
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_wrap_when_disabled() {
        let buffer = TextBuffer::new("line 1\nline 2\nline 3");
        let calc = WrappingCalculator::new(false, None);
        let visual_lines = calc.calculate_visual_lines(&buffer, 800.0);

        assert_eq!(visual_lines.len(), 3);
        assert_eq!(visual_lines[0].logical_line, 0);
        assert_eq!(visual_lines[1].logical_line, 1);
        assert_eq!(visual_lines[2].logical_line, 2);
    }

    #[test]
    fn test_wrap_at_fixed_column() {
        let buffer =
            TextBuffer::new("this is a very long line that should be wrapped");
        let calc = WrappingCalculator::new(true, Some(10));
        let visual_lines = calc.calculate_visual_lines(&buffer, 800.0);

        // Line is 47 chars, should wrap into 5 segments (10+10+10+10+7)
        assert_eq!(visual_lines.len(), 5);
        assert_eq!(visual_lines[0].start_col, 0);
        assert_eq!(visual_lines[0].end_col, 10);
        assert_eq!(visual_lines[1].start_col, 10);
        assert_eq!(visual_lines[1].end_col, 20);
        assert_eq!(visual_lines[4].start_col, 40);
        assert_eq!(visual_lines[4].end_col, 47);
    }

    #[test]
    fn test_logical_to_visual_mapping() {
        let buffer =
            TextBuffer::new("short\nthis is a very long line that wraps\nend");
        let calc = WrappingCalculator::new(true, Some(15));
        let visual_lines = calc.calculate_visual_lines(&buffer, 800.0);

        // First line (short) - no wrap
        assert_eq!(
            WrappingCalculator::logical_to_visual(&visual_lines, 0, 0),
            Some(0)
        );

        // Second line (long) - wraps
        assert_eq!(
            WrappingCalculator::logical_to_visual(&visual_lines, 1, 0),
            Some(1)
        );
        assert_eq!(
            WrappingCalculator::logical_to_visual(&visual_lines, 1, 14),
            Some(1)
        );
        assert_eq!(
            WrappingCalculator::logical_to_visual(&visual_lines, 1, 15),
            Some(2)
        );
        assert_eq!(
            WrappingCalculator::logical_to_visual(&visual_lines, 1, 30),
            Some(3)
        );
    }

    #[test]
    fn test_wrap_empty_lines() {
        let buffer = TextBuffer::new("line1\n\nline3");
        let calc = WrappingCalculator::new(true, Some(10));
        let visual_lines = calc.calculate_visual_lines(&buffer, 800.0);

        assert_eq!(visual_lines.len(), 3);
        assert_eq!(visual_lines[1].logical_line, 1);
        assert_eq!(visual_lines[1].len(), 0);
    }

    #[test]
    fn test_wrap_very_long_line() {
        let long_text = "a".repeat(100);
        let buffer = TextBuffer::new(&long_text);
        let calc = WrappingCalculator::new(true, Some(20));
        let visual_lines = calc.calculate_visual_lines(&buffer, 800.0);

        // 100 chars / 20 per line = 5 lines
        assert_eq!(visual_lines.len(), 5);
        assert!(visual_lines.iter().all(|vl| vl.logical_line == 0));
    }

    #[test]
    fn test_visual_line_is_first_segment() {
        let vl1 = VisualLine::new(0, 0, 0, 10);
        let vl2 = VisualLine::new(0, 1, 10, 20);

        assert!(vl1.is_first_segment());
        assert!(!vl2.is_first_segment());
    }
}
