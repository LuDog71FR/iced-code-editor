//! Cursor movement and positioning logic.

use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced::{Point, Task};

use super::{
    ArrowDirection, CHAR_WIDTH, CodeEditor, GUTTER_WIDTH, LINE_HEIGHT, Message,
    wrapping::WrappingCalculator,
};

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
                );
                let visual_lines = wrapping_calc
                    .calculate_visual_lines(&self.buffer, self.viewport_width);

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

    /// Handles mouse click to position cursor.
    pub(crate) fn handle_mouse_click(&mut self, point: Point) {
        // Account for gutter width
        if point.x < GUTTER_WIDTH {
            return; // Clicked in gutter
        }

        // Calculate visual line number based on viewport scroll
        let visual_line_idx =
            ((point.y + self.viewport_scroll) / LINE_HEIGHT) as usize;

        // Use wrapping calculator to find logical position
        let wrapping_calc =
            WrappingCalculator::new(self.wrap_enabled, self.wrap_column);
        let visual_lines = wrapping_calc
            .calculate_visual_lines(&self.buffer, self.viewport_width);

        if visual_line_idx >= visual_lines.len() {
            // Clicked beyond last line - move to end of document
            let last_line = self.buffer.line_count().saturating_sub(1);
            let last_col = self.buffer.line_len(last_line);
            self.cursor = (last_line, last_col);
            self.cache.clear();
            return;
        }

        let visual_line = &visual_lines[visual_line_idx];

        // Calculate column within the segment
        let x_in_text = point.x - GUTTER_WIDTH - 5.0;
        let col_offset = (x_in_text / CHAR_WIDTH).max(0.0) as usize;
        let col = (visual_line.start_col + col_offset).min(visual_line.end_col);

        self.cursor = (visual_line.logical_line, col);
        self.cache.clear();
    }

    /// Returns a scroll command to make the cursor visible.
    pub(crate) fn scroll_to_cursor(&self) -> Task<Message> {
        // Use wrapping calculator to find visual line
        let wrapping_calc =
            WrappingCalculator::new(self.wrap_enabled, self.wrap_column);
        let visual_lines = wrapping_calc
            .calculate_visual_lines(&self.buffer, self.viewport_width);

        let cursor_visual = WrappingCalculator::logical_to_visual(
            &visual_lines,
            self.cursor.0,
            self.cursor.1,
        );

        let cursor_y = if let Some(visual_idx) = cursor_visual {
            visual_idx as f32 * LINE_HEIGHT
        } else {
            // Fallback to logical line if visual not found
            self.cursor.0 as f32 * LINE_HEIGHT
        };

        let viewport_top = self.viewport_scroll;
        let viewport_bottom = self.viewport_scroll + self.viewport_height;

        // Add margins to avoid cursor being exactly at edge
        let top_margin = LINE_HEIGHT * 2.0;
        let bottom_margin = LINE_HEIGHT * 2.0;

        // Calculate new scroll position if cursor is outside visible area
        let new_scroll = if cursor_y < viewport_top + top_margin {
            // Cursor is above viewport - scroll up
            (cursor_y - top_margin).max(0.0)
        } else if cursor_y + LINE_HEIGHT > viewport_bottom - bottom_margin {
            // Cursor is below viewport - scroll down
            cursor_y + LINE_HEIGHT + bottom_margin - self.viewport_height
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
        let lines_per_page = (self.viewport_height / LINE_HEIGHT) as usize;

        let current_line = self.cursor.0;
        let new_line = current_line.saturating_sub(lines_per_page);
        let line_len = self.buffer.line_len(new_line);

        self.cursor = (new_line, self.cursor.1.min(line_len));
        self.cache.clear();
    }

    /// Moves cursor down by one page (approximately viewport height).
    pub(crate) fn page_down(&mut self) {
        let lines_per_page = (self.viewport_height / LINE_HEIGHT) as usize;

        let current_line = self.cursor.0;
        let max_line = self.buffer.line_count().saturating_sub(1);
        let new_line = (current_line + lines_per_page).min(max_line);
        let line_len = self.buffer.line_len(new_line);

        self.cursor = (new_line, self.cursor.1.min(line_len));
        self.cache.clear();
    }

    /// Handles mouse drag for text selection.
    pub(crate) fn handle_mouse_drag(&mut self, point: Point) {
        // Account for gutter width
        if point.x < GUTTER_WIDTH {
            return;
        }

        // Calculate visual line based on viewport scroll (same as click)
        let visual_line_idx =
            ((point.y + self.viewport_scroll) / LINE_HEIGHT) as usize;

        let wrapping_calc =
            WrappingCalculator::new(self.wrap_enabled, self.wrap_column);
        let visual_lines =
            wrapping_calc.calculate_visual_lines(&self.buffer, 800.0);

        if visual_line_idx >= visual_lines.len() {
            let last_line = self.buffer.line_count().saturating_sub(1);
            let last_col = self.buffer.line_len(last_line);
            self.cursor = (last_line, last_col);
            self.selection_end = Some(self.cursor);
            return;
        }

        let visual_line = &visual_lines[visual_line_idx];

        let x_in_text = point.x - GUTTER_WIDTH - 5.0;
        let col_offset = (x_in_text / CHAR_WIDTH).max(0.0) as usize;
        let col = (visual_line.start_col + col_offset).min(visual_line.end_col);

        // Update cursor and selection end
        self.cursor = (visual_line.logical_line, col);
        self.selection_end = Some(self.cursor);
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
}
