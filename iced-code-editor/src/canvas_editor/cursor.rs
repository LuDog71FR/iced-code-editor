//! Cursor movement and positioning logic.

use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced::{Point, Task};

use super::{
    ArrowDirection, CHAR_WIDTH, CanvasEditor, CanvasEditorMessage, GUTTER_WIDTH, LINE_HEIGHT,
};

impl CanvasEditor {
    /// Moves the cursor based on arrow key direction.
    pub(crate) fn move_cursor(&mut self, direction: ArrowDirection) {
        let (line, col) = self.cursor;

        match direction {
            ArrowDirection::Up => {
                if line > 0 {
                    let new_line = line - 1;
                    let line_len = self.buffer.line_len(new_line);
                    self.cursor = (new_line, col.min(line_len));
                }
            }
            ArrowDirection::Down => {
                if line + 1 < self.buffer.line_count() {
                    let new_line = line + 1;
                    let line_len = self.buffer.line_len(new_line);
                    self.cursor = (new_line, col.min(line_len));
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

        // Calculate line number
        let line = ((point.y + self.scroll_offset) / LINE_HEIGHT) as usize;
        let line = line.min(self.buffer.line_count().saturating_sub(1));

        // Calculate column
        let x_in_text = point.x - GUTTER_WIDTH;
        let col = (x_in_text / CHAR_WIDTH) as usize;
        let line_len = self.buffer.line_len(line);
        let col = col.min(line_len);

        self.cursor = (line, col);
        self.cache.clear();
    }

    /// Returns a scroll command to make the cursor visible.
    pub(crate) fn scroll_to_cursor(&self) -> Task<CanvasEditorMessage> {
        let cursor_y = self.cursor.0 as f32 * LINE_HEIGHT;
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
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: new_scroll,
            },
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

        // Calculate line and column (same as mouse click)
        let line = ((point.y + self.scroll_offset) / LINE_HEIGHT) as usize;
        let line = line.min(self.buffer.line_count().saturating_sub(1));

        let x_in_text = point.x - GUTTER_WIDTH;
        let col = (x_in_text / CHAR_WIDTH) as usize;
        let line_len = self.buffer.line_len(line);
        let col = col.min(line_len);

        // Update cursor and selection end
        self.cursor = (line, col);
        self.selection_end = Some(self.cursor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_movement() {
        let mut editor = CanvasEditor::new("line1\nline2", "py");
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
        let mut editor = CanvasEditor::new(&content, "py");

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
        let mut editor = CanvasEditor::new(&content, "py");

        // Move to line 50
        editor.cursor = (50, 0);
        editor.page_up();

        // Should move approximately 30 lines up
        assert!(editor.cursor.0 >= 15);
        assert!(editor.cursor.0 <= 25);
    }

    #[test]
    fn test_page_down_at_end() {
        let content = (0..10)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CanvasEditor::new(&content, "py");

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
        let mut editor = CanvasEditor::new(&content, "py");

        // Already at start
        editor.cursor = (0, 0);
        editor.page_up();
        assert_eq!(editor.cursor.0, 0);
    }
}
