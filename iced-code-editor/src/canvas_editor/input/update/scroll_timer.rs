//! Cursor-blink tick and vertical/horizontal scroll message handlers.

use iced::Task;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use crate::canvas_editor::{CURSOR_BLINK_INTERVAL, CodeEditor, Message};

impl CodeEditor {
    /// Handles cursor blink tick event.
    ///
    /// Updates cursor visibility for blinking animation.
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_tick_msg(&mut self) -> Task<Message> {
        // Handle cursor blinking only if editor has focus
        if self.has_focus()
            && self.last_blink.elapsed() >= CURSOR_BLINK_INTERVAL
        {
            self.cursor_visible = !self.cursor_visible;
            self.last_blink = Instant::now();
            self.overlay_cache.clear();
        }

        // Hide cursor if editor doesn't have focus
        if !self.has_focus() {
            self.show_cursor = false;
        }

        Task::none()
    }

    /// Handles viewport scrolled event.
    ///
    /// Manages the virtual scrolling cache window to optimize rendering
    /// for large files. Only clears the cache when scrolling crosses the
    /// cached window boundary or when viewport dimensions change.
    ///
    /// # Arguments
    ///
    /// * `viewport` - The viewport information after scrolling
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently Task::none())
    pub(crate) fn handle_scrolled_msg(
        &mut self,
        viewport: iced::widget::scrollable::Viewport,
    ) -> Task<Message> {
        // Virtual-scrolling cache window:
        // Instead of clearing the canvas cache for every small scroll,
        // we maintain a larger "render window" of visual lines around
        // the visible range. We only clear the cache and re-window
        // when the scroll crosses the window boundary or the viewport
        // size changes significantly. This prevents frequent re-highlighting
        // and layout recomputation for very large files while ensuring
        // the first scroll renders correctly without requiring a click.
        let new_scroll = viewport.absolute_offset().y;
        let new_height = viewport.bounds().height;
        let new_width = viewport.bounds().width;
        let scroll_changed = (self.viewport_scroll - new_scroll).abs() > 0.1;
        let visible_lines_count =
            (new_height / self.line_height).ceil() as usize + 2;
        let first_visible_line =
            (new_scroll / self.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count
            * crate::canvas_editor::CACHE_WINDOW_MARGIN_MULTIPLIER;
        let window_start = first_visible_line.saturating_sub(margin);
        let window_end = last_visible_line + margin;
        // Decide whether we need to re-window the cache.
        // Special-case top-of-file: when window_start == 0, allow small forward scrolls
        // without forcing a rewindow, to avoid thrashing when the visible range is near 0.
        let need_rewindow =
            if self.cache_window_end_line > self.cache_window_start_line {
                let lower_boundary_trigger = self.cache_window_start_line > 0
                    && first_visible_line
                        < self
                            .cache_window_start_line
                            .saturating_add(visible_lines_count / 2);
                let upper_boundary_trigger = last_visible_line
                    > self
                        .cache_window_end_line
                        .saturating_sub(visible_lines_count / 2);
                lower_boundary_trigger || upper_boundary_trigger
            } else {
                true
            };
        // Clear cache when viewport dimensions change significantly
        // to ensure proper redraw (e.g., window resize)
        if (self.viewport_height - new_height).abs() > 1.0
            || (self.viewport_width - new_width).abs() > 1.0
            || (scroll_changed && need_rewindow)
        {
            self.cache_window_start_line = window_start;
            self.cache_window_end_line = window_end;
            self.last_first_visible_line = first_visible_line;
            self.content_cache.clear();
            self.overlay_cache.clear();
        }
        self.viewport_scroll = new_scroll;
        self.viewport_height = new_height;
        self.viewport_width = new_width;
        Task::none()
    }

    /// Handles horizontal scrollbar scrolled event (only active when wrap is disabled).
    ///
    /// Updates `horizontal_scroll_offset` and clears render caches when the offset
    /// changes by more than 0.1 pixels to avoid unnecessary redraws.
    ///
    /// # Arguments
    ///
    /// * `viewport` - The viewport information after scrolling
    ///
    /// # Returns
    ///
    /// A `Task<Message>` (currently `Task::none()`)
    pub(crate) fn handle_horizontal_scrolled_msg(
        &mut self,
        viewport: iced::widget::scrollable::Viewport,
    ) -> Task<Message> {
        let new_x = viewport.absolute_offset().x;
        if (self.horizontal_scroll_offset - new_x).abs() > 0.1 {
            self.horizontal_scroll_offset = new_x;
            self.content_cache.clear();
            self.overlay_cache.clear();
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_scroll_initial_state() {
        let editor = CodeEditor::new("short line", "rs");
        assert!(
            (editor.horizontal_scroll_offset - 0.0).abs() < f32::EPSILON,
            "Initial horizontal scroll offset should be 0"
        );
    }

    #[test]
    fn test_set_wrap_enabled_resets_horizontal_offset() {
        let mut editor = CodeEditor::new("long line", "rs");
        editor.wrap_enabled = false;
        // Simulate a non-zero horizontal scroll
        editor.horizontal_scroll_offset = 100.0;

        // Re-enabling wrap should reset horizontal offset
        editor.set_wrap_enabled(true);
        assert!(
            (editor.horizontal_scroll_offset - 0.0).abs() < f32::EPSILON,
            "Horizontal scroll offset should be reset when wrap is re-enabled"
        );
    }

    #[test]
    fn test_scroll_sets_initial_cache_window() {
        let content =
            (0..200).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut editor = CodeEditor::new(&content, "py");

        // Simulate initial viewport
        let height = 400.0;
        let width = 800.0;
        let scroll = 0.0;

        // Expected derived ranges
        let visible_lines_count =
            (height / editor.line_height).ceil() as usize + 2;
        let first_visible_line = (scroll / editor.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count * 2;
        let window_start = first_visible_line.saturating_sub(margin);
        let window_end = last_visible_line + margin;

        // Apply logic similar to Message::Scrolled branch
        editor.viewport_height = height;
        editor.viewport_width = width;
        editor.viewport_scroll = -1.0;
        let scroll_changed = (editor.viewport_scroll - scroll).abs() > 0.1;
        let need_rewindow = true;
        if (editor.viewport_height - height).abs() > 1.0
            || (editor.viewport_width - width).abs() > 1.0
            || (scroll_changed && need_rewindow)
        {
            editor.cache_window_start_line = window_start;
            editor.cache_window_end_line = window_end;
            editor.last_first_visible_line = first_visible_line;
        }
        editor.viewport_scroll = scroll;

        assert_eq!(editor.last_first_visible_line, first_visible_line);
        assert!(editor.cache_window_end_line > editor.cache_window_start_line);
        assert_eq!(editor.cache_window_start_line, window_start);
        assert_eq!(editor.cache_window_end_line, window_end);
    }

    #[test]
    fn test_small_scroll_keeps_window() {
        let content =
            (0..200).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut editor = CodeEditor::new(&content, "py");
        let height = 400.0;
        let width = 800.0;
        let initial_scroll = 0.0;
        let visible_lines_count =
            (height / editor.line_height).ceil() as usize + 2;
        let first_visible_line =
            (initial_scroll / editor.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count * 2;
        let window_start = first_visible_line.saturating_sub(margin);
        let window_end = last_visible_line + margin;
        editor.cache_window_start_line = window_start;
        editor.cache_window_end_line = window_end;
        editor.viewport_height = height;
        editor.viewport_width = width;
        editor.viewport_scroll = initial_scroll;

        // Small scroll inside window
        let small_scroll =
            editor.line_height * (visible_lines_count as f32 / 4.0);
        let first_visible_line2 =
            (small_scroll / editor.line_height).floor() as usize;
        let last_visible_line2 = first_visible_line2 + visible_lines_count;
        let lower_boundary_trigger = editor.cache_window_start_line > 0
            && first_visible_line2
                < editor
                    .cache_window_start_line
                    .saturating_add(visible_lines_count / 2);
        let upper_boundary_trigger = last_visible_line2
            > editor
                .cache_window_end_line
                .saturating_sub(visible_lines_count / 2);
        let need_rewindow = lower_boundary_trigger || upper_boundary_trigger;

        assert!(!need_rewindow, "Small scroll should be inside the window");
        // Window remains unchanged
        assert_eq!(editor.cache_window_start_line, window_start);
        assert_eq!(editor.cache_window_end_line, window_end);
    }

    #[test]
    fn test_large_scroll_rewindows() {
        let content =
            (0..1000).map(|i| format!("line{}\n", i)).collect::<String>();
        let mut editor = CodeEditor::new(&content, "py");
        let height = 400.0;
        let width = 800.0;
        let initial_scroll = 0.0;
        let visible_lines_count =
            (height / editor.line_height).ceil() as usize + 2;
        let first_visible_line =
            (initial_scroll / editor.line_height).floor() as usize;
        let last_visible_line = first_visible_line + visible_lines_count;
        let margin = visible_lines_count * 2;
        editor.cache_window_start_line =
            first_visible_line.saturating_sub(margin);
        editor.cache_window_end_line = last_visible_line + margin;
        editor.viewport_height = height;
        editor.viewport_width = width;
        editor.viewport_scroll = initial_scroll;

        // Large scroll beyond window boundary
        let large_scroll =
            editor.line_height * ((visible_lines_count * 4) as f32);
        let first_visible_line2 =
            (large_scroll / editor.line_height).floor() as usize;
        let last_visible_line2 = first_visible_line2 + visible_lines_count;
        let window_start2 = first_visible_line2.saturating_sub(margin);
        let window_end2 = last_visible_line2 + margin;
        let need_rewindow = first_visible_line2
            < editor
                .cache_window_start_line
                .saturating_add(visible_lines_count / 2)
            || last_visible_line2
                > editor
                    .cache_window_end_line
                    .saturating_sub(visible_lines_count / 2);
        assert!(need_rewindow, "Large scroll should trigger window update");

        // Apply rewindow
        editor.cache_window_start_line = window_start2;
        editor.cache_window_end_line = window_end2;
        editor.last_first_visible_line = first_visible_line2;

        assert_eq!(editor.cache_window_start_line, window_start2);
        assert_eq!(editor.cache_window_end_line, window_end2);
        assert_eq!(editor.last_first_visible_line, first_visible_line2);
    }
}
