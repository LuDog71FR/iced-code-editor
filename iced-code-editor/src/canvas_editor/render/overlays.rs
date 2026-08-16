//! Overlay-layer rendering: current-line highlight, search/selection/bracket
//! highlights, and cursor/caret drawing for [`CodeEditor`].

use iced::mouse;
use iced::widget::canvas;
use iced::{Color, Point, Rectangle, Size};

use super::text::{RenderContext, calculate_segment_geometry};
use super::wrapping::{VisualLine, WrappingCalculator};
use crate::canvas_editor::vim::VimMode;
use crate::canvas_editor::{
    CodeEditor, measure_char_width, measure_text_width,
};
use crate::canvas_editor::{bracket_match, search};

impl CodeEditor {
    /// Draws the background highlight for the current line.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_line` - The visual line to check
    /// * `y` - Y position for rendering
    pub(super) fn draw_current_line_highlight(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
    ) {
        if self.cursors.iter().any(|c| c.position.0 == visual_line.logical_line)
        {
            frame.fill_rectangle(
                Point::new(ctx.gutter_width, y),
                Size::new(ctx.bounds_width - ctx.gutter_width, ctx.line_height),
                self.style.current_line_highlight,
            );
        }
    }

    /// Fills a single highlight rectangle for a column range within one visual
    /// line.
    ///
    /// Computes the CJK-aware segment geometry, applies the horizontal scroll
    /// offset, and draws the rectangle inset vertically to match the editor's
    /// highlight styling. Shared by selection and search-match rendering.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_idx` - Index of the visual line being drawn (drives the Y position)
    /// * `vl` - The visual line whose segment is highlighted
    /// * `cols` - Inclusive start and exclusive end columns of the segment
    /// * `color` - Fill color of the highlight rectangle
    fn fill_highlight_segment(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_idx: usize,
        vl: &VisualLine,
        cols: (usize, usize),
        color: Color,
    ) {
        let y = visual_idx as f32 * ctx.line_height;
        let line_content = self.buffer.line(vl.logical_line);
        let (x_start, width) = calculate_segment_geometry(
            line_content,
            vl.start_col,
            cols.0,
            cols.1,
            ctx.gutter_width + 5.0,
            ctx.full_char_width,
            ctx.char_width,
        );
        let x_start = x_start - ctx.horizontal_scroll_offset;
        frame.fill_rectangle(
            Point::new(x_start, y + 2.0),
            Size::new(width, ctx.line_height - 4.0),
            color,
        );
    }

    /// Draws search match highlights for all visible matches.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `first_visible_line` - First visible visual line index
    /// * `last_visible_line` - Last visible visual line index
    pub(super) fn draw_search_highlights(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        start_visual_idx: usize,
        end_visual_idx: usize,
    ) {
        if !self.search_matches_visible() || self.search_state.query.is_empty()
        {
            return;
        }

        let query_len = self.search_state.query.chars().count();

        let start_visual_idx = start_visual_idx.min(ctx.visual_lines.len());
        let end_visual_idx = end_visual_idx.min(ctx.visual_lines.len());

        let end_visual_inclusive = end_visual_idx
            .saturating_sub(1)
            .min(ctx.visual_lines.len().saturating_sub(1));

        if let (Some(start_vl), Some(end_vl)) = (
            ctx.visual_lines.get(start_visual_idx),
            ctx.visual_lines.get(end_visual_inclusive),
        ) {
            let min_logical_line = start_vl.logical_line;
            let max_logical_line = end_vl.logical_line;

            // Optimization: Use get_visible_match_range to find matches in view
            // This uses binary search + early termination for O(log N) performance
            let match_range = search::get_visible_match_range(
                &self.search_state.matches,
                min_logical_line,
                max_logical_line,
            );

            for (match_idx, search_match) in self
                .search_state
                .matches
                .iter()
                .enumerate()
                .skip(match_range.start)
                .take(match_range.len())
            {
                // Determine if this is the current match
                let is_current =
                    self.search_state.current_match_index == Some(match_idx);

                let highlight_color = if is_current {
                    self.style.search_match_current_color
                } else {
                    self.style.search_match_color
                };

                // Convert logical position to visual line
                let start_visual = WrappingCalculator::logical_to_visual(
                    ctx.visual_lines,
                    search_match.line,
                    search_match.col,
                );
                let end_visual = WrappingCalculator::logical_to_visual(
                    ctx.visual_lines,
                    search_match.line,
                    search_match.col + query_len,
                );

                if let (Some(start_v), Some(end_v)) = (start_visual, end_visual)
                {
                    if start_v == end_v {
                        // Match within same visual line
                        let vl = &ctx.visual_lines[start_v];
                        self.fill_highlight_segment(
                            frame,
                            ctx,
                            start_v,
                            vl,
                            (search_match.col, search_match.col + query_len),
                            highlight_color,
                        );
                    } else {
                        // Match spans multiple visual lines
                        for (v_idx, vl) in ctx
                            .visual_lines
                            .iter()
                            .enumerate()
                            .skip(start_v)
                            .take(end_v - start_v + 1)
                        {
                            let sel_start_col = if v_idx == start_v {
                                search_match.col
                            } else {
                                vl.start_col
                            };
                            let sel_end_col = if v_idx == end_v {
                                search_match.col + query_len
                            } else {
                                vl.end_col
                            };

                            self.fill_highlight_segment(
                                frame,
                                ctx,
                                v_idx,
                                vl,
                                (sel_start_col, sel_end_col),
                                highlight_color,
                            );
                        }
                    }
                }
            }
        }
    }

    /// Draws the selection highlight for a single cursor range.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `start` - Selection start (line, col)
    /// * `end` - Selection end (line, col), must be >= start
    fn draw_single_selection(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        start: (usize, usize),
        end: (usize, usize),
    ) {
        let selection_color = self.style.selection_color;

        if start.0 == end.0 {
            // Single line selection - need to handle wrapped segments
            let start_visual = WrappingCalculator::logical_to_visual(
                ctx.visual_lines,
                start.0,
                start.1,
            );
            let end_visual = WrappingCalculator::logical_to_visual(
                ctx.visual_lines,
                end.0,
                end.1,
            );

            if let (Some(start_v), Some(end_v)) = (start_visual, end_visual) {
                if start_v == end_v {
                    // Selection within same visual line
                    let vl = &ctx.visual_lines[start_v];
                    self.fill_highlight_segment(
                        frame,
                        ctx,
                        start_v,
                        vl,
                        (start.1, end.1),
                        selection_color,
                    );
                } else {
                    // Selection spans multiple visual lines (same logical line)
                    for (v_idx, vl) in ctx
                        .visual_lines
                        .iter()
                        .enumerate()
                        .skip(start_v)
                        .take(end_v - start_v + 1)
                    {
                        let sel_start_col = if v_idx == start_v {
                            start.1
                        } else {
                            vl.start_col
                        };
                        let sel_end_col =
                            if v_idx == end_v { end.1 } else { vl.end_col };

                        self.fill_highlight_segment(
                            frame,
                            ctx,
                            v_idx,
                            vl,
                            (sel_start_col, sel_end_col),
                            selection_color,
                        );
                    }
                }
            }
        } else {
            // Multi-line selection
            let start_visual = WrappingCalculator::logical_to_visual(
                ctx.visual_lines,
                start.0,
                start.1,
            );
            let end_visual = WrappingCalculator::logical_to_visual(
                ctx.visual_lines,
                end.0,
                end.1,
            );

            if let (Some(start_v), Some(end_v)) = (start_visual, end_visual) {
                for (v_idx, vl) in ctx
                    .visual_lines
                    .iter()
                    .enumerate()
                    .skip(start_v)
                    .take(end_v - start_v + 1)
                {
                    let sel_start_col =
                        if vl.logical_line == start.0 && v_idx == start_v {
                            start.1
                        } else {
                            vl.start_col
                        };

                    let sel_end_col =
                        if vl.logical_line == end.0 && v_idx == end_v {
                            end.1
                        } else {
                            vl.end_col
                        };

                    self.fill_highlight_segment(
                        frame,
                        ctx,
                        v_idx,
                        vl,
                        (sel_start_col, sel_end_col),
                        selection_color,
                    );
                }
            }
        }
    }

    /// Draws a highlight around the bracket or quote pair touching the
    /// primary cursor, if any.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    pub(super) fn draw_matching_bracket_highlight(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
    ) {
        if !self.bracket_match_highlight_enabled {
            return;
        }

        let Some((bracket_pos, match_pos)) = bracket_match::find_matching_pair(
            &self.buffer,
            self.cursors.primary_position(),
        ) else {
            return;
        };

        let bracket_color = self.style.bracket_match_color;

        for (line, col) in [bracket_pos, match_pos] {
            if let Some(visual_idx) = WrappingCalculator::logical_to_visual(
                ctx.visual_lines,
                line,
                col,
            ) {
                let vl = &ctx.visual_lines[visual_idx];
                self.fill_highlight_segment(
                    frame,
                    ctx,
                    visual_idx,
                    vl,
                    (col, col + 1),
                    bracket_color,
                );
            }
        }
    }

    /// Draws text selection highlights for all cursors.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    pub(super) fn draw_selection_highlight(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
    ) {
        for cursor in self.cursors.iter() {
            if let Some((start, end)) = cursor.selection_range()
                && start != end
            {
                self.draw_single_selection(frame, ctx, start, end);
            }
        }
    }

    /// Draws the cursor (normal caret or IME preedit cursor).
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    pub(super) fn draw_cursor(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
    ) {
        // Cursor drawing logic (only when the editor has focus)
        // -------------------------------------------------------------------------
        // Core notes:
        // 1. Choose the drawing path based on whether IME preedit is present.
        // 2. Require both `is_focused()` (Iced focus) and `has_canvas_focus()` (internal focus)
        //    so the cursor is drawn only in the active editor, avoiding multiple cursors.
        // 3. Use `WrappingCalculator` to map logical (line, col) to visual (x, y)
        //    for correct cursor positioning with line wrapping.
        // -------------------------------------------------------------------------
        if self.show_cursor
            && self.cursor_visible
            && self.has_focus()
            && self.ime_preedit.is_some()
        {
            // [Branch A] IME preedit rendering mode
            // ---------------------------------------------------------------------
            // When the user is composing with an IME (e.g. pinyin before commit),
            // draw a preedit region instead of the normal caret, including:
            // - preedit background (highlighting the composing text)
            // - preedit text content (preedit.content)
            // - preedit selection (underline or selection background)
            // - preedit caret
            // ---------------------------------------------------------------------
            if let Some(cursor_visual) = WrappingCalculator::logical_to_visual(
                ctx.visual_lines,
                self.cursors.primary_position().0,
                self.cursors.primary_position().1,
            ) {
                let vl = &ctx.visual_lines[cursor_visual];
                let line_content = self.buffer.line(vl.logical_line);

                // Compute the preedit region start X
                // Use calculate_segment_geometry to ensure correct CJK width handling
                let (cursor_x_content, _) = calculate_segment_geometry(
                    line_content,
                    vl.start_col,
                    self.cursors.primary_position().1,
                    self.cursors.primary_position().1,
                    ctx.gutter_width + 5.0,
                    ctx.full_char_width,
                    ctx.char_width,
                );
                let cursor_x = cursor_x_content - ctx.horizontal_scroll_offset;
                let cursor_y = cursor_visual as f32 * ctx.line_height;

                if let Some(preedit) = self.ime_preedit.as_ref() {
                    let preedit_width = measure_text_width(
                        &preedit.content,
                        ctx.full_char_width,
                        ctx.char_width,
                    );

                    // 1. Draw preedit background (light translucent)
                    // This indicates the text is not committed yet
                    frame.fill_rectangle(
                        Point::new(cursor_x, cursor_y + 2.0),
                        Size::new(preedit_width, ctx.line_height - 4.0),
                        self.style.ime_preedit_background_color,
                    );

                    // 2. Draw preedit selection (if any)
                    // IME may mark a selection inside preedit text (e.g. segmentation)
                    // The range uses UTF-8 byte indices, so slices must be safe
                    if let Some(range) = preedit.selection.as_ref()
                        && range.start != range.end
                    {
                        // Validate indices before slicing to prevent panic
                        if let Some((start, end)) = validate_selection_indices(
                            &preedit.content,
                            range.start,
                            range.end,
                        ) {
                            let selected_prefix = &preedit.content[..start];
                            let selected_text = &preedit.content[start..end];

                            let selection_x = cursor_x
                                + measure_text_width(
                                    selected_prefix,
                                    ctx.full_char_width,
                                    ctx.char_width,
                                );
                            let selection_w = measure_text_width(
                                selected_text,
                                ctx.full_char_width,
                                ctx.char_width,
                            );

                            frame.fill_rectangle(
                                Point::new(selection_x, cursor_y + 2.0),
                                Size::new(selection_w, ctx.line_height - 4.0),
                                self.style.selection_color,
                            );
                        }
                    }

                    // 3. Draw preedit text itself
                    frame.fill_text(canvas::Text {
                        content: preedit.content.clone(),
                        position: Point::new(cursor_x, cursor_y + 2.0),
                        color: self.style.text_color,
                        size: ctx.font_size.into(),
                        font: ctx.font,
                        ..canvas::Text::default()
                    });

                    // 4. Draw bottom underline (IME state indicator)
                    frame.fill_rectangle(
                        Point::new(cursor_x, cursor_y + ctx.line_height - 3.0),
                        Size::new(preedit_width, 1.0),
                        self.style.text_color,
                    );

                    // 5. Draw preedit caret
                    // If IME provides a caret position (usually selection end), draw a thin bar
                    if let Some(range) = preedit.selection.as_ref() {
                        let caret_end = range.end.min(preedit.content.len());

                        // Validate caret position to avoid panic on invalid UTF-8 boundary
                        if caret_end <= preedit.content.len()
                            && preedit.content.is_char_boundary(caret_end)
                        {
                            let caret_prefix = &preedit.content[..caret_end];
                            let caret_x = cursor_x
                                + measure_text_width(
                                    caret_prefix,
                                    ctx.full_char_width,
                                    ctx.char_width,
                                );

                            frame.fill_rectangle(
                                Point::new(caret_x, cursor_y + 2.0),
                                Size::new(2.0, ctx.line_height - 4.0),
                                self.style.text_color,
                            );
                        }
                    }
                }
            }
        } else if self.show_cursor && self.cursor_visible && self.has_focus() {
            // [Branch B] Normal caret rendering mode
            // ---------------------------------------------------------------------
            // Vim mode is single-cursor and Visual selections use an inclusive
            // active position that differs from the editor's half-open cursor.
            // Standard editing continues to render every cursor in the set.
            // ---------------------------------------------------------------------
            if self.vim_enabled {
                let position = self
                    .vim_state
                    .visual_positions()
                    .map(|(_, active)| active)
                    .unwrap_or_else(|| self.cursors.primary_position());
                self.draw_single_caret(frame, ctx, position);
            } else {
                for cursor in self.cursors.iter() {
                    self.draw_single_caret(frame, ctx, cursor.position);
                }
            }
        }
    }

    /// Returns the cursor size for a logical position using current font metrics.
    ///
    /// Standard editing and Vim Insert mode use the existing 2px bar. Vim
    /// Normal and Visual modes use the width of the character under the cursor;
    /// an empty line or end-of-line position uses one narrow character width.
    fn cursor_size_for_position(&self, position: (usize, usize)) -> Size {
        let uses_block =
            self.vim_enabled && self.vim_state.mode() != VimMode::Insert;
        let width = if uses_block {
            self.buffer
                .line(position.0)
                .chars()
                .nth(position.1)
                .map(|ch| {
                    measure_char_width(
                        ch,
                        self.full_char_width,
                        self.char_width,
                    )
                })
                .filter(|width| *width > 0.0)
                .unwrap_or(self.char_width)
        } else {
            2.0
        };

        Size::new(width, (self.line_height - 4.0).max(1.0))
    }

    /// Draws one cursor at the given logical (line, col) position.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `position` - Logical cursor position (line, col)
    fn draw_single_caret(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        position: (usize, usize),
    ) {
        // Map logical cursor position (line, col) to visual line index
        if let Some(cursor_visual) = WrappingCalculator::logical_to_visual(
            ctx.visual_lines,
            position.0,
            position.1,
        ) {
            let vl = &ctx.visual_lines[cursor_visual];
            let line_content = self.buffer.line(vl.logical_line);

            // Compute exact caret X position
            let (cursor_x_content, _) = calculate_segment_geometry(
                line_content,
                vl.start_col,
                position.1,
                position.1,
                ctx.gutter_width + 5.0,
                ctx.full_char_width,
                ctx.char_width,
            );
            let cursor_x = cursor_x_content - ctx.horizontal_scroll_offset;
            let cursor_y = cursor_visual as f32 * ctx.line_height;

            let cursor_size = self.cursor_size_for_position(position);
            let mut cursor_color = self.style.text_color;
            if cursor_size.width > 2.0 {
                cursor_color.a *= 0.55;
            }

            frame.fill_rectangle(
                Point::new(cursor_x, cursor_y + 2.0),
                cursor_size,
                cursor_color,
            );
        }
    }

    /// Draws underlines for jumpable links when modifier is held.
    pub(super) fn draw_jump_link_highlight(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) {
        #[cfg(target_os = "macos")]
        let modifier_active = self.modifiers.get().command();
        #[cfg(not(target_os = "macos"))]
        let modifier_active = self.modifiers.get().control();

        if !modifier_active {
            return;
        }

        let Some(point) = cursor.position_in(bounds) else {
            return;
        };

        if let Some((line, col)) = self.calculate_cursor_from_point(point) {
            let line_content = self.buffer.line(line);

            let start_col = Self::word_start_in_line(line_content, col);
            let end_col = Self::word_end_in_line(line_content, col);

            if start_col >= end_col {
                return;
            }

            // Find the first visual line for this logical line
            if let Some(mut idx) =
                WrappingCalculator::logical_to_visual(ctx.visual_lines, line, 0)
            {
                // Iterate all visual lines belonging to this logical line
                while idx < ctx.visual_lines.len() {
                    let visual_line = &ctx.visual_lines[idx];
                    if visual_line.logical_line != line {
                        break;
                    }

                    // Check intersection
                    let seg_start = visual_line.start_col.max(start_col);
                    let seg_end = visual_line.end_col.min(end_col);

                    if seg_start < seg_end {
                        let (x, width) = calculate_segment_geometry(
                            line_content,
                            visual_line.start_col,
                            seg_start,
                            seg_end,
                            ctx.gutter_width + 5.0
                                - ctx.horizontal_scroll_offset,
                            ctx.full_char_width,
                            ctx.char_width,
                        );

                        let y = idx as f32 * ctx.line_height + ctx.line_height; // Underline at bottom

                        // Draw underline
                        let path = canvas::Path::line(
                            Point::new(x, y),
                            Point::new(x + width, y),
                        );

                        frame.stroke(
                            &path,
                            canvas::Stroke::default()
                                .with_color(self.style.text_color) // Use text color or link color
                                .with_width(1.0),
                        );
                    }

                    idx += 1;
                }
            }
        }
    }
}

/// Validates that the selection indices fall on valid UTF-8 character boundaries
/// to prevent panics during string slicing.
///
/// # Arguments
///
/// * `content` - The string content to check against
/// * `start` - The start byte index
/// * `end` - The end byte index
///
/// # Returns
///
/// `Some((start, end))` if indices are valid, `None` otherwise.
fn validate_selection_indices(
    content: &str,
    start: usize,
    end: usize,
) -> Option<(usize, usize)> {
    let len = content.len();
    // Clamp indices to content length
    let start = start.min(len);
    let end = end.min(len);

    // Ensure start is not greater than end
    if start > end {
        return None;
    }

    // Verify that indices fall on valid UTF-8 character boundaries
    if content.is_char_boundary(start) && content.is_char_boundary(end) {
        Some((start, end))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;
    use crate::canvas_editor::compare_floats;

    #[test]
    fn test_vim_cursor_rendering_insert_uses_bar() {
        let mut editor = CodeEditor::new("a", "txt").with_vim_enabled(true);
        let _ = editor.vim_state.parse_key('i');

        let size = editor.cursor_size_for_position((0, 0));

        assert_eq!(compare_floats(size.width, 2.0), Ordering::Equal);
        assert_eq!(
            compare_floats(size.height, editor.line_height() - 4.0),
            Ordering::Equal
        );
    }

    #[test]
    fn test_vim_cursor_rendering_normal_uses_ascii_block() {
        let editor = CodeEditor::new("a", "txt").with_vim_enabled(true);

        let size = editor.cursor_size_for_position((0, 0));

        assert_eq!(
            compare_floats(size.width, editor.char_width()),
            Ordering::Equal
        );
    }

    #[test]
    fn test_vim_cursor_rendering_normal_uses_cjk_width() {
        let editor = CodeEditor::new("汉", "txt").with_vim_enabled(true);

        let size = editor.cursor_size_for_position((0, 0));

        assert_eq!(
            compare_floats(size.width, editor.full_char_width()),
            Ordering::Equal
        );
    }

    #[test]
    fn test_vim_cursor_rendering_empty_line_has_visible_block() {
        let editor = CodeEditor::new("", "txt").with_vim_enabled(true);

        let size = editor.cursor_size_for_position((0, 0));

        assert_eq!(
            compare_floats(size.width, editor.char_width()),
            Ordering::Equal
        );
        assert!(size.width > 2.0);
    }

    #[test]
    fn test_validate_selection_indices() {
        // Test valid ASCII indices
        let content = "Hello";
        assert_eq!(validate_selection_indices(content, 0, 5), Some((0, 5)));
        assert_eq!(validate_selection_indices(content, 1, 3), Some((1, 3)));

        // Test valid multi-byte indices (Chinese "你好")
        // "你" is 3 bytes (0-3), "好" is 3 bytes (3-6)
        let content = "你好";
        assert_eq!(validate_selection_indices(content, 0, 6), Some((0, 6)));
        assert_eq!(validate_selection_indices(content, 0, 3), Some((0, 3)));
        assert_eq!(validate_selection_indices(content, 3, 6), Some((3, 6)));

        // Test invalid indices (splitting multi-byte char)
        assert_eq!(validate_selection_indices(content, 1, 3), None); // Split first char
        assert_eq!(validate_selection_indices(content, 0, 4), None); // Split second char

        // Test out of bounds (should be clamped if on boundary, but here len is 6)
        // If we pass start=0, end=100 -> clamped to 0, 6. 6 is boundary.
        assert_eq!(validate_selection_indices(content, 0, 100), Some((0, 6)));

        // Test inverted range
        assert_eq!(validate_selection_indices(content, 3, 0), None);
    }
}
