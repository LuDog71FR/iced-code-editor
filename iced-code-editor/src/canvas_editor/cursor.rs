//! Cursor movement and positioning logic.

use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced::{Point, Task};

use super::wrapping::WrappingCalculator;
use super::{ArrowDirection, CodeEditor, Message};

impl CodeEditor {
    /// Moves the cursor based on arrow key direction.
    pub(crate) fn move_cursor(&mut self, direction: ArrowDirection) {
        let (line, col) = self.cursor;

        match direction {
            ArrowDirection::Up | ArrowDirection::Down => {
                // For up/down, we need to handle wrapped lines
                let wrapping_calc = WrappingCalculator::new(
                    self.wrap_enabled,
                    self.wrap_column,
                    self.full_char_width,
                    self.char_width,
                );
                let visual_lines = wrapping_calc.calculate_visual_lines(
                    &self.buffer,
                    self.viewport_width,
                    self.gutter_width(),
                );

                // Find current visual line
                if let Some(current_visual) =
                    WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        line,
                        col,
                    )
                {
                    let target_visual = match direction {
                        ArrowDirection::Up => {
                            if current_visual > 0 {
                                current_visual - 1
                            } else {
                                return; // Already at top
                            }
                        }
                        ArrowDirection::Down => {
                            if current_visual + 1 < visual_lines.len() {
                                current_visual + 1
                            } else {
                                return; // Already at bottom
                            }
                        }
                        _ => {
                            // This should never happen as we're in the Up/Down branch
                            return;
                        }
                    };

                    let target_vl = &visual_lines[target_visual];
                    let current_vl = &visual_lines[current_visual];

                    // Try to maintain column position, clamped to segment
                    let new_col = if target_vl.logical_line == line {
                        // Same logical line, different segment
                        // Calculate relative position in current segment
                        let offset_in_current =
                            col.saturating_sub(current_vl.start_col);
                        // Apply to target segment, ensuring we stay within bounds
                        let target_col =
                            target_vl.start_col + offset_in_current;
                        // Clamp to segment bounds: stay strictly within [start_col, end_col)
                        // but make sure we don't go to exactly end_col unless it's the last segment
                        if target_col >= target_vl.end_col {
                            target_vl
                                .end_col
                                .saturating_sub(1)
                                .max(target_vl.start_col)
                        } else {
                            target_col
                        }
                    } else {
                        // Different logical line
                        let target_line_len =
                            self.buffer.line_len(target_vl.logical_line);
                        (target_vl.start_col + col.min(target_vl.len()))
                            .min(target_line_len)
                    };

                    self.cursor = (target_vl.logical_line, new_col);
                }
            }
            ArrowDirection::Left => {
                if col > 0 {
                    self.cursor.1 -= 1;
                } else if line > 0 {
                    // Move to end of previous line
                    let prev_line_len = self.buffer.line_len(line - 1);
                    self.cursor = (line - 1, prev_line_len);
                }
            }
            ArrowDirection::Right => {
                let line_len = self.buffer.line_len(line);
                if col < line_len {
                    self.cursor.1 += 1;
                } else if line + 1 < self.buffer.line_count() {
                    // Move to start of next line
                    self.cursor = (line + 1, 0);
                }
            }
        }
        self.cache.clear();
    }

    /// Computes the cursor logical position (line, col) from a screen point.
    ///
    /// This method considers:
    /// 1. Whether the click is inside the gutter area.
    /// 2. Visual line mapping after wrapping.
    /// 3. CJK character widths (wide characters use FONT_SIZE, narrow use CHAR_WIDTH).
    fn calculate_cursor_from_point(
        &self,
        point: Point,
    ) -> Option<(usize, usize)> {
        // Account for gutter width
        if point.x < self.gutter_width() {
            return None; // Clicked in gutter
        }

        // Calculate visual line number - point.y is already in canvas coordinates
        let visual_line_idx = (point.y / self.line_height) as usize;

        // Use wrapping calculator to find logical position
        let wrapping_calc = WrappingCalculator::new(
            self.wrap_enabled,
            self.wrap_column,
            self.full_char_width,
            self.char_width,
        );
        let visual_lines = wrapping_calc.calculate_visual_lines(
            &self.buffer,
            self.viewport_width,
            self.gutter_width(),
        );

        if visual_line_idx >= visual_lines.len() {
            // Clicked beyond last line - move to end of document
            let last_line = self.buffer.line_count().saturating_sub(1);
            let last_col = self.buffer.line_len(last_line);
            return Some((last_line, last_col));
        }

        let visual_line = &visual_lines[visual_line_idx];

        // Calculate column within the segment
        let x_in_text = point.x - self.gutter_width() - 5.0;

        // Use correct width calculation for CJK support
        let line_content = self.buffer.line(visual_line.logical_line);
        let segment_text: String = line_content
            .chars()
            .skip(visual_line.start_col)
            .take(visual_line.end_col - visual_line.start_col)
            .collect();

        let mut current_width = 0.0;
        let mut col_offset = 0;

        for c in segment_text.chars() {
            let char_width = super::measure_char_width(
                c,
                self.full_char_width,
                self.char_width,
            );

            if current_width + char_width / 2.0 > x_in_text {
                break;
            }
            current_width += char_width;
            col_offset += 1;
        }

        let col = visual_line.start_col + col_offset;
        Some((visual_line.logical_line, col))
    }

    /// Handles mouse clicks to position the cursor.
    ///
    /// Reuses `calculate_cursor_from_point` to compute the position and updates the cache.
    pub(crate) fn handle_mouse_click(&mut self, point: Point) {
        if let Some(cursor) = self.calculate_cursor_from_point(point) {
            self.cursor = cursor;
            self.cache.clear();
        }
    }

    /// Returns a scroll command to make the cursor visible.
    pub(crate) fn scroll_to_cursor(&self) -> Task<Message> {
        // Use wrapping calculator to find visual line
        let wrapping_calc = WrappingCalculator::new(
            self.wrap_enabled,
            self.wrap_column,
            self.full_char_width,
            self.char_width,
        );
        let visual_lines = wrapping_calc.calculate_visual_lines(
            &self.buffer,
            self.viewport_width,
            self.gutter_width(),
        );

        let cursor_visual = WrappingCalculator::logical_to_visual(
            &visual_lines,
            self.cursor.0,
            self.cursor.1,
        );

        let cursor_y = if let Some(visual_idx) = cursor_visual {
            visual_idx as f32 * self.line_height
        } else {
            // Fallback to logical line if visual not found
            self.cursor.0 as f32 * self.line_height
        };

        let viewport_top = self.viewport_scroll;
        let viewport_bottom = self.viewport_scroll + self.viewport_height;

        // Add margins to avoid cursor being exactly at edge
        let top_margin = self.line_height * 2.0;
        let bottom_margin = self.line_height * 2.0;

        // Calculate new scroll position if cursor is outside visible area
        let new_scroll = if cursor_y < viewport_top + top_margin {
            // Cursor is above viewport - scroll up
            (cursor_y - top_margin).max(0.0)
        } else if cursor_y + self.line_height > viewport_bottom - bottom_margin
        {
            // Cursor is below viewport - scroll down
            cursor_y + self.line_height + bottom_margin - self.viewport_height
        } else {
            // Cursor is visible - no scroll needed
            return Task::none();
        };

        scroll_to(
            self.scrollable_id.clone(),
            scrollable::AbsoluteOffset { x: 0.0, y: new_scroll },
        )
    }

    /// Moves cursor up by one page (approximately viewport height).
    pub(crate) fn page_up(&mut self) {
        let lines_per_page = (self.viewport_height / self.line_height) as usize;

        let current_line = self.cursor.0;
        let new_line = current_line.saturating_sub(lines_per_page);
        let line_len = self.buffer.line_len(new_line);

        self.cursor = (new_line, self.cursor.1.min(line_len));
        self.cache.clear();
    }

    /// Moves cursor down by one page (approximately viewport height).
    pub(crate) fn page_down(&mut self) {
        let lines_per_page = (self.viewport_height / self.line_height) as usize;

        let current_line = self.cursor.0;
        let max_line = self.buffer.line_count().saturating_sub(1);
        let new_line = (current_line + lines_per_page).min(max_line);
        let line_len = self.buffer.line_len(new_line);

        self.cursor = (new_line, self.cursor.1.min(line_len));
        self.cache.clear();
    }

    /// Handles mouse drag for text selection.
    ///
    /// Reuses `calculate_cursor_from_point` to compute the position and update selection end.
    pub(crate) fn handle_mouse_drag(&mut self, point: Point) {
        if let Some(cursor) = self.calculate_cursor_from_point(point) {
            self.cursor = cursor;
            self.selection_end = Some(self.cursor);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_movement() {
        let mut editor = CodeEditor::new("line1\nline2", "py");
        editor.move_cursor(ArrowDirection::Down);
        assert_eq!(editor.cursor.0, 1);
        editor.move_cursor(ArrowDirection::Right);
        assert_eq!(editor.cursor.1, 1);
    }

    #[test]
    fn test_page_down() {
        // Create editor with many lines
        let content = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "py");

        editor.page_down();
        // Should move approximately 30 lines (600px / 20px per line)
        assert!(editor.cursor.0 >= 25);
        assert!(editor.cursor.0 <= 35);
    }

    #[test]
    fn test_page_up() {
        // Create editor with many lines
        let content = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "py");

        // Move to line 50
        editor.cursor = (50, 0);
        editor.page_up();

        // Should move approximately 30 lines up
        assert!(editor.cursor.0 >= 15);
        assert!(editor.cursor.0 <= 25);
    }

    #[test]
    fn test_page_down_at_end() {
        let content =
            (0..10).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let mut editor = CodeEditor::new(&content, "py");

        editor.page_down();
        // Should be at last line (line 9)
        assert_eq!(editor.cursor.0, 9);
    }

    #[test]
    fn test_page_up_at_start() {
        let content = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "py");

        // Already at start
        editor.cursor = (0, 0);
        editor.page_up();
        assert_eq!(editor.cursor.0, 0);
    }

    #[test]
    fn test_cursor_click_cjk() {
        use iced::Point;
        let mut editor = CodeEditor::new("你好", "txt");
        editor.set_line_numbers_enabled(false);

        let full_char_width = editor.full_char_width();
        let half_width = full_char_width / 2.0;
        let padding = 5.0;

        // Assume each CJK character is `full_char_width` wide.
        // "你" is 0..full_char_width. "好" is full_char_width..2*full_char_width.
        //
        // Case 1: Click inside "你", at less than half its width.
        // Expect col 0
        editor
            .handle_mouse_click(Point::new((half_width - 2.0) + padding, 10.0));

        assert_eq!(editor.cursor, (0, 0));

        // Case 2: Click inside "你", at more than half its width.
        // Expect col 1
        editor
            .handle_mouse_click(Point::new((half_width + 2.0) + padding, 10.0));
        assert_eq!(editor.cursor, (0, 1));

        // Case 3: Click inside "好", at less than half its width.
        // "好" starts at full_char_width. Offset into "好" is < half_width.
        // Expect col 1 (start of "好")
        editor.handle_mouse_click(Point::new(
            (full_char_width + half_width - 2.0) + padding,
            10.0,
        ));
        assert_eq!(editor.cursor, (0, 1));

        // Case 4: Click inside "好", at more than half its width.
        // "好" starts at full_char_width. Offset into "好" is > half_width.
        // Expect col 2 (end of "好")
        editor.handle_mouse_click(Point::new(
            (full_char_width + half_width + 2.0) + padding,
            10.0,
        ));
        assert_eq!(editor.cursor, (0, 2));
    }
}
