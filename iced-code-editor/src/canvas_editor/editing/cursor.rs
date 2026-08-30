//! Cursor movement and positioning logic.

use iced::widget::operation::scroll_to;
use iced::widget::scrollable;
use iced::{Point, Task};
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use super::cursor_set;
use crate::buffer::TextBuffer;
use crate::canvas_editor::features::vim::VimMotion;
use crate::canvas_editor::render::wrapping::{VisualLine, WrappingCalculator};
use crate::canvas_editor::{
    ArrowDirection, CodeEditor, Message, measure_char_width, measure_text_width,
};
/// Maps a cursor's column onto the visual row it is moving to.
///
/// Within one logical line the cursor keeps its offset inside the row, so it
/// stays under the same on-screen column; crossing into another logical line
/// it keeps that offset but is clamped to what the target line actually holds.
///
/// Shared by arrow Up/Down and by Page Up/Down: both land a cursor on a
/// different visual row and need the same answer to "which column now?".
///
/// # Arguments
///
/// * `pos` - The cursor's current logical position
/// * `current_vl` - The visual row the cursor is leaving
/// * `target_vl` - The visual row the cursor is landing on
/// * `buffer` - The text buffer, for the target line's length
///
/// # Returns
///
/// The column the cursor takes on `target_vl`.
fn column_on_visual_row(
    pos: (usize, usize),
    current_vl: &VisualLine,
    target_vl: &VisualLine,
    buffer: &TextBuffer,
) -> usize {
    let (line, col) = pos;
    if target_vl.logical_line == line {
        let offset_in_current = col.saturating_sub(current_vl.start_col);
        let target_col = target_vl.start_col + offset_in_current;
        if target_col >= target_vl.end_col {
            target_vl.end_col.saturating_sub(1).max(target_vl.start_col)
        } else {
            target_col
        }
    } else {
        let target_line_len = buffer.line_len(target_vl.logical_line);
        (target_vl.start_col + col.min(target_vl.len())).min(target_line_len)
    }
}

/// Computes the next logical `(line, col)` position for a cursor at `pos` moving in `direction`.
///
/// Returns `None` if the cursor is already at the boundary and cannot move further.
fn compute_next_position(
    pos: (usize, usize),
    direction: ArrowDirection,
    buffer: &TextBuffer,
    visual_lines: &[VisualLine],
) -> Option<(usize, usize)> {
    let (line, col) = pos;
    match direction {
        ArrowDirection::Up | ArrowDirection::Down => {
            let current_visual =
                WrappingCalculator::logical_to_visual(visual_lines, line, col)?;

            let target_visual = match direction {
                ArrowDirection::Up => current_visual.checked_sub(1)?,
                ArrowDirection::Down => {
                    let next = current_visual + 1;
                    if next < visual_lines.len() {
                        next
                    } else {
                        return None;
                    }
                }
                _ => return None,
            };

            let target_vl = &visual_lines[target_visual];
            let current_vl = &visual_lines[current_visual];

            Some((
                target_vl.logical_line,
                column_on_visual_row(pos, current_vl, target_vl, buffer),
            ))
        }
        ArrowDirection::Left => {
            if col > 0 {
                Some((line, col - 1))
            } else if line > 0 {
                Some((line - 1, buffer.line_len(line - 1)))
            } else {
                None
            }
        }
        ArrowDirection::Right => {
            let line_len = buffer.line_len(line);
            if col < line_len {
                Some((line, col + 1))
            } else if line + 1 < buffer.line_count() {
                Some((line + 1, 0))
            } else {
                None
            }
        }
    }
}

impl CodeEditor {
    /// Clamps a logical position to a character on which Normal/Visual mode can
    /// land. Non-empty lines use their final character as the maximum column;
    /// empty lines retain column zero.
    pub(crate) fn vim_normal_position(
        &self,
        position: (usize, usize),
    ) -> (usize, usize) {
        let line = position.0.min(self.buffer.line_count().saturating_sub(1));
        let line_len = self.buffer.line_len(line);
        let max_col = line_len.saturating_sub(1);
        (line, position.1.min(max_col))
    }

    fn vim_position_after(&self, position: (usize, usize)) -> (usize, usize) {
        let line_len = self.buffer.line_len(position.0);
        if line_len == 0 {
            if position.0 + 1 < self.buffer.line_count() {
                (position.0 + 1, 0)
            } else {
                position
            }
        } else {
            (position.0, (position.1 + 1).min(line_len))
        }
    }

    /// Projects an inclusive Vim Visual selection into the editor's half-open
    /// cursor selection representation.
    pub(crate) fn apply_vim_visual_selection(
        &mut self,
        anchor: (usize, usize),
        active: (usize, usize),
        linewise: bool,
    ) {
        let anchor = self.vim_normal_position(anchor);
        let active = self.vim_normal_position(active);
        let cursor = if linewise {
            let start_line = anchor.0.min(active.0);
            let end_line = anchor.0.max(active.0);
            let end = if end_line + 1 < self.buffer.line_count() {
                (end_line + 1, 0)
            } else {
                (end_line, self.buffer.line_len(end_line))
            };
            cursor_set::Cursor { position: end, anchor: Some((start_line, 0)) }
        } else if active >= anchor {
            cursor_set::Cursor {
                position: self.vim_position_after(active),
                anchor: Some(anchor),
            }
        } else {
            cursor_set::Cursor {
                position: active,
                anchor: Some(self.vim_position_after(anchor)),
            }
        };
        self.cursors.set_single(cursor.position);
        self.cursors.primary_mut().anchor = cursor.anchor;
        self.overlay_cache.clear();
    }

    /// Resolves one Vim motion from a character position, including counted
    /// visible-line movement and Unicode-aware word boundaries.
    pub(crate) fn vim_motion_target(
        &self,
        start: (usize, usize),
        motion: VimMotion,
        count: usize,
        explicit_count: bool,
    ) -> (usize, usize) {
        let mut position = self.vim_normal_position(start);
        let count = count.max(1);

        match motion {
            VimMotion::Left => {
                position.1 = position.1.saturating_sub(count);
            }
            VimMotion::Right => {
                let max_col =
                    self.buffer.line_len(position.0).saturating_sub(1);
                position.1 = position.1.saturating_add(count).min(max_col);
            }
            VimMotion::Up | VimMotion::Down => {
                let direction = if motion == VimMotion::Up {
                    ArrowDirection::Up
                } else {
                    ArrowDirection::Down
                };
                let visual_lines =
                    self.visual_lines_cached(self.viewport_width);
                for _ in 0..count {
                    let Some(next) = compute_next_position(
                        position,
                        direction,
                        &self.buffer,
                        &visual_lines,
                    ) else {
                        break;
                    };
                    position = self.vim_normal_position(next);
                }
            }
            VimMotion::WordForward
            | VimMotion::WordBackward
            | VimMotion::WordEnd => {
                let chars = self.vim_char_index();
                for _ in 0..count {
                    position = Self::vim_word_motion(&chars, position, motion);
                }
            }
            VimMotion::LineStart => position.1 = 0,
            VimMotion::FirstNonBlank => {
                position.1 = self
                    .buffer
                    .line(position.0)
                    .chars()
                    .position(|ch| !ch.is_whitespace())
                    .unwrap_or(0);
            }
            VimMotion::LineEnd => {
                position.1 = self.buffer.line_len(position.0).saturating_sub(1);
            }
            VimMotion::DocumentStart => {
                let line = count
                    .saturating_sub(1)
                    .min(self.buffer.line_count().saturating_sub(1));
                position = (line, 0);
            }
            VimMotion::DocumentEnd => {
                let line = if explicit_count {
                    count
                        .saturating_sub(1)
                        .min(self.buffer.line_count().saturating_sub(1))
                } else {
                    self.buffer.line_count().saturating_sub(1)
                };
                position = (line, 0);
            }
        }

        self.vim_normal_position(position)
    }

    /// Builds a flat, position-tagged character index across the whole
    /// buffer, inserting a synthetic `'\n'` at each line boundary so that
    /// word motions can cross line breaks. Rebuilding this once per keystroke
    /// (rather than once per counted repetition) keeps counted motions
    /// (`5w`) linear in document size instead of `O(document_size * count)`.
    fn vim_char_index(&self) -> Vec<((usize, usize), char)> {
        let mut chars = Vec::new();
        for line in 0..self.buffer.line_count() {
            chars.extend(
                self.buffer
                    .line(line)
                    .chars()
                    .enumerate()
                    .map(|(col, ch)| ((line, col), ch)),
            );
            if line + 1 < self.buffer.line_count() {
                chars.push(((line, self.buffer.line_len(line)), '\n'));
            }
        }
        chars
    }

    /// Resolves a single word-wise Vim motion (`w`/`b`/`e`) against a
    /// prebuilt character index (see [`Self::vim_char_index`]).
    fn vim_word_motion(
        chars: &[((usize, usize), char)],
        start: (usize, usize),
        motion: VimMotion,
    ) -> (usize, usize) {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Class {
            Space,
            Word,
            Punctuation,
        }

        fn class(ch: char) -> Class {
            if ch.is_whitespace() {
                Class::Space
            } else if ch.is_alphanumeric() || ch == '_' {
                Class::Word
            } else {
                Class::Punctuation
            }
        }

        if chars.is_empty() {
            return (0, 0);
        }

        let insertion =
            chars.partition_point(|(position, _)| *position < start);
        let exact = insertion < chars.len() && chars[insertion].0 == start;

        match motion {
            VimMotion::WordForward => {
                let mut index = insertion;
                if exact {
                    let current_class = class(chars[index].1);
                    if current_class == Class::Space {
                        while index < chars.len()
                            && class(chars[index].1) == Class::Space
                        {
                            index += 1;
                        }
                    } else {
                        while index < chars.len()
                            && class(chars[index].1) == current_class
                        {
                            index += 1;
                        }
                        while index < chars.len()
                            && class(chars[index].1) == Class::Space
                        {
                            index += 1;
                        }
                    }
                }
                chars[index.min(chars.len() - 1)].0
            }
            VimMotion::WordBackward => {
                let mut index = insertion.saturating_sub(1);
                while index > 0 && class(chars[index].1) == Class::Space {
                    index -= 1;
                }
                let target_class = class(chars[index].1);
                while index > 0 && class(chars[index - 1].1) == target_class {
                    index -= 1;
                }
                chars[index].0
            }
            VimMotion::WordEnd => {
                let mut index = insertion.min(chars.len() - 1);
                if exact {
                    let current_class = class(chars[index].1);
                    if current_class != Class::Space
                        && index + 1 < chars.len()
                        && class(chars[index + 1].1) == current_class
                    {
                        while index + 1 < chars.len()
                            && class(chars[index + 1].1) == current_class
                        {
                            index += 1;
                        }
                        return chars[index].0;
                    }
                    index = (index + 1).min(chars.len() - 1);
                }
                while index + 1 < chars.len()
                    && class(chars[index].1) == Class::Space
                {
                    index += 1;
                }
                let target_class = class(chars[index].1);
                while index + 1 < chars.len()
                    && class(chars[index + 1].1) == target_class
                {
                    index += 1;
                }
                chars[index].0
            }
            _ => start,
        }
    }

    /// Sets the cursor position to the specified line and column.
    ///
    /// This method ensures the new position is within the bounds of the text buffer.
    /// It also resets the blinking animation, clears the overlay cache (to redraw
    /// the cursor immediately), and scrolls the view to make the cursor visible.
    ///
    /// # Arguments
    ///
    /// * `line` - The target line index (0-based).
    /// * `col` - The target column index (0-based).
    ///
    /// # Returns
    ///
    /// A `Task` that may produce a `Message` (e.g., if scrolling is needed).
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
    ///
    /// let _task = editor.set_cursor(1, 2);
    /// assert_eq!(editor.cursor_position(), (1, 2));
    ///
    /// // Out-of-range coordinates clamp to the document rather than failing,
    /// // which is what makes this safe to call with a position from an
    /// // external source such as an LSP jump target.
    /// let _task = editor.set_cursor(99, 99);
    /// assert_eq!(editor.cursor_position(), (2, 5));
    /// ```
    pub fn set_cursor(&mut self, line: usize, col: usize) -> Task<Message> {
        let line = line.min(self.buffer.line_count().saturating_sub(1));
        let line_len = self.buffer.line(line).chars().count();
        let col = col.min(line_len);

        self.cursors.set_single((line, col));
        // Programmatic jumps should end any drag gesture. Otherwise, a stale
        // drag state may let subsequent hover events move the caret away.
        self.is_dragging = false;

        // Reset blink
        self.last_blink = Instant::now();

        self.overlay_cache.clear();
        self.scroll_to_cursor()
    }

    /// Moves all cursors one step in `direction`.
    ///
    /// Visual lines are computed once and shared across all cursor movements.
    /// After moving, overlapping cursors are merged via `sort_and_merge`.
    pub(crate) fn move_cursor(&mut self, direction: ArrowDirection) {
        // Compute visual lines once — used by Up/Down movement for all cursors.
        // Reuse the memoized layout so that lines hidden by collapsed folds are
        // skipped during vertical navigation, exactly like in rendering.
        let visual_lines = self.visual_lines_cached(self.viewport_width);

        for cursor in self.cursors.as_mut_slice() {
            if let Some(new_pos) = compute_next_position(
                cursor.position,
                direction,
                &self.buffer,
                &visual_lines,
            ) {
                cursor.position = new_pos;
            }
        }

        // Deduplicate cursors that landed on the same position after movement.
        self.cursors.sort_and_merge();

        // Cursor movement affects only overlay visuals (caret, current-line highlight),
        // so avoid invalidating the expensive content cache.
        self.overlay_cache.clear();
    }

    /// Computes the cursor logical position (line, col) from a screen point.
    ///
    /// This method considers:
    /// 1. Whether the click is inside the gutter area.
    /// 2. Visual line mapping after wrapping.
    /// 3. CJK character widths (wide characters use FONT_SIZE, narrow use CHAR_WIDTH).
    pub(crate) fn calculate_cursor_from_point(
        &self,
        point: Point,
    ) -> Option<(usize, usize)> {
        // Account for gutter width
        if point.x < self.gutter_width() {
            return None; // Clicked in gutter
        }

        // Calculate visual line number - point.y is already in canvas coordinates
        let visual_line_idx = (point.y / self.line_height) as usize;

        // Reuse memoized wrapping result for hit-testing. This avoids recomputing
        // visual lines on every mouse move/drag.
        let visual_lines = self.visual_lines_cached(self.viewport_width);

        if visual_line_idx >= visual_lines.len() {
            // Clicked beyond last line - move to end of document
            let last_line = self.buffer.line_count().saturating_sub(1);
            let last_col = self.buffer.line_len(last_line);
            return Some((last_line, last_col));
        }

        let visual_line = &visual_lines[visual_line_idx];

        // Calculate column within the segment, accounting for horizontal scroll
        let x_in_text =
            point.x - self.gutter_width() - 5.0 + self.horizontal_scroll_offset;

        // Use correct width calculation for CJK support
        let line_content = self.buffer.line(visual_line.logical_line);

        let mut current_width = 0.0;
        let mut col_offset = 0;

        // Iterate the visual slice directly to avoid allocating a temporary String.
        for c in line_content
            .chars()
            .skip(visual_line.start_col)
            .take(visual_line.end_col - visual_line.start_col)
        {
            let char_width =
                measure_char_width(c, self.full_char_width, self.char_width);

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
        let before = self.cursors.primary_position();
        if let Some(pos) = self.calculate_cursor_from_point(point) {
            self.cursors.primary_mut().position = pos;
            if self.cursors.primary_position() != before {
                // Only clear overlay when the caret actually moved.
                self.overlay_cache.clear();
            }
        }
    }

    /// Classifies a left-button press as a single/double/triple click.
    ///
    /// Consecutive presses count up as long as each one lands within 400ms
    /// otherwise the count resets to 1. Counts
    /// wrap back to 1 after 3, so a fourth rapid click starts a fresh
    /// single/double/triple cycle rather than being silently ignored.
    pub(crate) fn classify_click(&self, position: Point) -> u8 {
        let now = Instant::now();
        let count = match self.last_click.get() {
            Some((time, pos, count))
                if now.duration_since(time)
                    < std::time::Duration::from_millis(400)
                    && pos.distance(position) < 6.0 =>
            {
                if count >= 3 {
                    1
                } else {
                    count + 1
                }
            }
            _ => 1,
        };
        self.last_click.set(Some((now, position, count)));
        count
    }

    /// Returns a scroll command placing `line` `rows_above` rows below the top
    /// of the viewport.
    ///
    /// Unlike [`Self::scroll_to_cursor`], this scrolls unconditionally and
    /// leaves the cursor untouched: it backs the sticky-scroll headers, where
    /// clicking a pinned header is a navigation gesture, not an edit.
    ///
    /// `rows_above` is what reserves room for anything drawn *over* the top of
    /// the viewport. Scrolling a line to row 0 only makes it visible when
    /// nothing covers row 0, which is not the case once sticky-scroll headers
    /// are pinned there — see [`CodeEditor::sticky_headroom`].
    ///
    /// # Arguments
    ///
    /// * `line` - Index of the logical line to bring into view
    /// * `rows_above` - Rows to leave free above it; `0` puts it at the very top
    ///
    /// # Returns
    ///
    /// A `Task<Message>` scrolling the editor, or `Task::none()` when `line` is
    /// hidden by a collapsed fold or out of bounds
    pub(crate) fn scroll_to_line(
        &self,
        line: usize,
        rows_above: usize,
    ) -> Task<Message> {
        let visual_lines = self.visual_lines_cached(self.viewport_width);

        let Some(visual_index) =
            WrappingCalculator::logical_to_visual(&visual_lines, line, 0)
        else {
            return Task::none();
        };

        // Saturating: a line already within `rows_above` of the top of the
        // buffer cannot be pushed down, and scrolling to a negative offset is
        // not a thing.
        let first_visible = visual_index.saturating_sub(rows_above);

        scroll_to(
            self.scrollable_id.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: first_visible as f32 * self.line_height,
            },
        )
    }

    /// Returns a scroll command to make the cursor visible.
    pub(crate) fn scroll_to_cursor(&self) -> Task<Message> {
        // Reuse memoized wrapping result so repeated scroll computations do not
        // trigger repeated visual line calculation.
        let visual_lines = self.visual_lines_cached(self.viewport_width);

        let pos = self.cursors.primary_position();
        let cursor_visual =
            WrappingCalculator::logical_to_visual(&visual_lines, pos.0, pos.1);

        let cursor_y = if let Some(visual_idx) = cursor_visual {
            visual_idx as f32 * self.line_height
        } else {
            // Fallback to logical line if visual not found
            pos.0 as f32 * self.line_height
        };

        let viewport_top = self.viewport_scroll;
        let viewport_bottom = self.viewport_scroll + self.viewport_height;

        // Add margins to avoid cursor being exactly at edge
        let top_margin = self.line_height * 2.0;
        let bottom_margin = self.line_height * 2.0;

        // Calculate new vertical scroll position if cursor is outside visible area
        let new_v_scroll = if cursor_y < viewport_top + top_margin {
            // Cursor is above viewport - scroll up
            Some((cursor_y - top_margin).max(0.0))
        } else if cursor_y + self.line_height > viewport_bottom - bottom_margin
        {
            // Cursor is below viewport - scroll down
            Some(
                cursor_y + self.line_height + bottom_margin
                    - self.viewport_height,
            )
        } else {
            None
        };

        let vertical_task = if let Some(new_scroll) = new_v_scroll {
            scroll_to(
                self.scrollable_id.clone(),
                scrollable::AbsoluteOffset { x: 0.0, y: new_scroll },
            )
        } else {
            Task::none()
        };

        // Horizontal scroll: only when wrap is disabled
        let h_task = if !self.wrap_enabled {
            // Compute cursor content-space X position
            let cursor_content_x = if let Some(visual_idx) = cursor_visual {
                let vl = &visual_lines[visual_idx];
                let line_content = self.buffer.line(vl.logical_line);
                let prefix: String = line_content
                    .chars()
                    .skip(vl.start_col)
                    .take(pos.1.saturating_sub(vl.start_col))
                    .collect();
                self.gutter_width()
                    + 5.0
                    + measure_text_width(
                        &prefix,
                        self.full_char_width,
                        self.char_width,
                    )
            } else {
                self.gutter_width() + 5.0
            };

            let left_boundary = self.gutter_width() + self.char_width;
            let right_boundary = self.viewport_width - self.char_width * 2.0;
            let cursor_viewport_x =
                cursor_content_x - self.horizontal_scroll_offset;

            let new_h_offset = if cursor_viewport_x < left_boundary {
                (cursor_content_x - left_boundary).max(0.0)
            } else if cursor_viewport_x > right_boundary {
                cursor_content_x - right_boundary
            } else {
                self.horizontal_scroll_offset // no change
            };

            if (new_h_offset - self.horizontal_scroll_offset).abs() > 0.5 {
                scroll_to(
                    self.horizontal_scrollable_id.clone(),
                    scrollable::AbsoluteOffset { x: new_h_offset, y: 0.0 },
                )
            } else {
                Task::none()
            }
        } else {
            Task::none()
        };

        Task::batch([vertical_task, h_task])
    }

    /// Returns how many whole text rows the viewport shows.
    ///
    /// The unit is the *visual* row, the one [`Self::visual_lines_cached`]
    /// returns: with wrapping enabled one logical line occupies several of
    /// them, so a page measured in logical lines would be as many times too
    /// long as the lines it crosses happen to wrap.
    ///
    /// # Returns
    ///
    /// The number of complete rows that fit in the viewport.
    fn rows_per_page(&self) -> usize {
        (self.viewport_height / self.line_height) as usize
    }

    /// Moves every cursor to the visual row given by `map_row`, clamped to the
    /// last row, then merges overlapping cursors and invalidates the overlay
    /// cache.
    ///
    /// Working in visual rows is what keeps a page one screenful: it is the
    /// same space the viewport is measured in, wrapped segments and lines
    /// hidden by a collapsed fold included, so the cursor travels exactly as
    /// far as the view does. A cursor sitting on a line the layout does not
    /// emit — hidden inside a collapsed fold — has no row to move from and
    /// stays where it is.
    ///
    /// Shared by [`page_up`](Self::page_up) and [`page_down`](Self::page_down).
    ///
    /// # Arguments
    ///
    /// * `map_row` - Maps a cursor's current visual row to its target row.
    fn move_cursors_by_visual_row(&mut self, map_row: impl Fn(usize) -> usize) {
        let visual_lines = self.visual_lines_cached(self.viewport_width);
        let Some(last_row) = visual_lines.len().checked_sub(1) else {
            return; // No layout at all: nothing to page through.
        };

        for cursor in self.cursors.as_mut_slice() {
            let Some(current_row) = WrappingCalculator::logical_to_visual(
                &visual_lines,
                cursor.position.0,
                cursor.position.1,
            ) else {
                continue;
            };

            let target_row = map_row(current_row).min(last_row);
            cursor.position = (
                visual_lines[target_row].logical_line,
                column_on_visual_row(
                    cursor.position,
                    &visual_lines[current_row],
                    &visual_lines[target_row],
                    &self.buffer,
                ),
            );
        }

        self.cursors.sort_and_merge();
        self.overlay_cache.clear();
    }

    /// Moves all cursors up by one viewport height.
    pub(crate) fn page_up(&mut self) {
        let rows = self.rows_per_page();
        self.move_cursors_by_visual_row(|row| row.saturating_sub(rows));
    }

    /// Moves all cursors down by one viewport height.
    pub(crate) fn page_down(&mut self) {
        let rows = self.rows_per_page();
        self.move_cursors_by_visual_row(|row| row.saturating_add(rows));
    }

    /// Handles mouse drag for text selection.
    ///
    /// Reuses `calculate_cursor_from_point` to compute the position and update selection end.
    pub(crate) fn handle_mouse_drag(&mut self, point: Point) {
        if let Some(pos) = self.calculate_cursor_from_point(point) {
            self.cursors.primary_mut().position = pos;
        }
    }
}

impl CodeEditor {
    /// Converts a logical buffer position into a canvas point, if visible.
    pub(crate) fn point_from_position(
        &self,
        line: usize,
        col: usize,
    ) -> Option<iced::Point> {
        let visual_lines = self.visual_lines_cached(self.viewport_width);
        let visual_index =
            WrappingCalculator::logical_to_visual(&visual_lines, line, col)?;
        let visual_line = &visual_lines[visual_index];
        let line_content = self.buffer.line(visual_line.logical_line);
        let prefix_len = col.saturating_sub(visual_line.start_col);
        let prefix_text: String = line_content
            .chars()
            .skip(visual_line.start_col)
            .take(prefix_len)
            .collect();
        let x = self.gutter_width()
            + 5.0
            + measure_text_width(
                &prefix_text,
                self.full_char_width,
                self.char_width,
            );
        let y = visual_index as f32 * self.line_height;
        Some(iced::Point::new(x, y))
    }

    /// Returns the word-start column in a line for a given column.
    ///
    /// Word characters include ASCII alphanumerics and underscore.
    pub(crate) fn word_start_in_line(line: &str, col: usize) -> usize {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return 0;
        }
        let mut idx = col.min(chars.len());
        if idx == chars.len() {
            idx = idx.saturating_sub(1);
        }
        if !Self::is_word_char(chars[idx]) {
            if idx > 0 && Self::is_word_char(chars[idx - 1]) {
                idx -= 1;
            } else {
                return col.min(chars.len());
            }
        }
        while idx > 0 && Self::is_word_char(chars[idx - 1]) {
            idx -= 1;
        }
        idx
    }

    /// Returns the word-end column in a line for a given column.
    pub(crate) fn word_end_in_line(line: &str, col: usize) -> usize {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            return 0;
        }
        let mut idx = col.min(chars.len());
        if idx == chars.len() {
            idx = idx.saturating_sub(1);
        }

        // If current char is not a word char, check if previous was (we might be just after the word)
        if !Self::is_word_char(chars[idx]) {
            if idx > 0 && Self::is_word_char(chars[idx - 1]) {
                // We are just after a word, so idx is the end (exclusive)
                // But wait, if we are at the space after "foo", idx points to space.
                // "foo " -> ' ' is at 3. word_end should be 3.
                // So if chars[idx] is not word char, and chars[idx-1] IS, then idx is the end.
                return idx;
            } else {
                // Not on a word
                return col.min(chars.len());
            }
        }

        // If we are on a word char, scan forward
        while idx < chars.len() && Self::is_word_char(chars[idx]) {
            idx += 1;
        }
        idx
    }

    /// Returns true when the character is part of an identifier-style word.
    pub(crate) fn is_word_char(ch: char) -> bool {
        ch == '_' || ch.is_alphanumeric()
    }

    /// Returns the screen position of the cursor.
    ///
    /// This method returns the (x, y) coordinates of the current cursor position
    /// relative to the editor canvas, accounting for gutter width and line height.
    ///
    /// # Returns
    ///
    /// An `Option<iced::Point>` containing the cursor position, or `None` if
    /// the cursor position cannot be determined.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// if let Some(point) = editor.cursor_screen_position() {
    ///     println!("Cursor at: ({}, {})", point.x, point.y);
    /// }
    /// ```
    pub fn cursor_screen_position(&self) -> Option<iced::Point> {
        let pos = self.cursors.primary_position();
        self.point_from_position(pos.0, pos.1)
    }

    /// Returns the current cursor position as (line, column).
    ///
    /// This method returns the logical cursor position in the buffer,
    /// where line and column are both 0-indexed.
    ///
    /// # Returns
    ///
    /// A tuple `(line, column)` representing the cursor position.
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// let (line, col) = editor.cursor_position();
    /// println!("Cursor at line {}, column {}", line, col);
    /// ```
    pub fn cursor_position(&self) -> (usize, usize) {
        self.cursors.primary_position()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_word_start_in_line() {
        let line = "foo_bar baz";
        assert_eq!(CodeEditor::word_start_in_line(line, 0), 0);
        assert_eq!(CodeEditor::word_start_in_line(line, 2), 0);
        assert_eq!(CodeEditor::word_start_in_line(line, 4), 0);
        assert_eq!(CodeEditor::word_start_in_line(line, 7), 0);
        assert_eq!(CodeEditor::word_start_in_line(line, 9), 8);
    }

    #[test]
    fn vim_word_motion_crosses_line_boundary_via_prebuilt_index() {
        let editor = CodeEditor::new("one two\nthree four", "txt");
        let chars = editor.vim_char_index();

        assert_eq!(
            CodeEditor::vim_word_motion(&chars, (0, 4), VimMotion::WordForward),
            (1, 0)
        );
        assert_eq!(
            CodeEditor::vim_word_motion(
                &chars,
                (1, 0),
                VimMotion::WordBackward
            ),
            (0, 4)
        );
        assert_eq!(
            CodeEditor::vim_word_motion(&chars, (0, 4), VimMotion::WordEnd),
            (0, 6)
        );
    }

    #[test]
    fn test_cursor_movement() {
        let mut editor = CodeEditor::new("line1\nline2", "py");
        editor.move_cursor(ArrowDirection::Down);
        assert_eq!(editor.cursors.primary_position().0, 1);
        editor.move_cursor(ArrowDirection::Right);
        assert_eq!(editor.cursors.primary_position().1, 1);
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
        assert!(editor.cursors.primary_position().0 >= 25);
        assert!(editor.cursors.primary_position().0 <= 35);
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
        editor.cursors.primary_mut().position = (50, 0);
        editor.page_up();

        // Should move approximately 30 lines up
        assert!(editor.cursors.primary_position().0 >= 15);
        assert!(editor.cursors.primary_position().0 <= 25);
    }

    #[test]
    fn test_page_down_at_end() {
        let content =
            (0..10).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let mut editor = CodeEditor::new(&content, "py");

        editor.page_down();
        // Should be at last line (line 9)
        assert_eq!(editor.cursors.primary_position().0, 9);
    }

    /// Builds an editor whose logical lines each wrap over several visual rows,
    /// with a viewport exactly three rows tall. Paging it by logical lines and
    /// paging it by visual rows give visibly different answers, which is what
    /// these tests are for.
    fn wrapped_editor() -> CodeEditor {
        let long = "x".repeat(400);
        let content =
            (0..10).map(|_| long.clone()).collect::<Vec<_>>().join("\n");
        let mut editor = CodeEditor::new(&content, "py");
        editor.viewport_height = editor.line_height * 3.0;
        editor.viewport_width = 300.0;
        editor
    }

    #[test]
    fn test_page_down_moves_one_viewport_of_visual_rows() {
        // Regression test. Paging used to add `viewport_height / line_height`
        // to the *logical* line index, so with wrapping on -- the default --
        // one press moved the cursor as many screenfuls as the lines it
        // crossed happened to wrap: 45 rows here, in a viewport showing 3.
        let mut editor = wrapped_editor();
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);
        let rows_in_first_line =
            visual_lines.iter().filter(|row| row.logical_line == 0).count();
        assert!(
            rows_in_first_line > 3,
            "the fixture only tests anything if its lines really wrap"
        );

        // The cursor starts on visual row 0, which is (0, 0) by construction.
        editor.cursors.primary_mut().position = (0, 0);
        editor.page_down();

        let target = &visual_lines[3];
        assert_eq!(
            editor.cursors.primary_position(),
            (target.logical_line, target.start_col)
        );
        // Three rows into a line that wraps is still that same line.
        assert_eq!(editor.cursors.primary_position().0, 0);
    }

    #[test]
    fn test_page_up_moves_one_viewport_of_visual_rows() {
        let mut editor = wrapped_editor();
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);

        let start = &visual_lines[20];
        editor.cursors.primary_mut().position =
            (start.logical_line, start.start_col);
        editor.page_up();

        let target = &visual_lines[17];
        assert_eq!(
            editor.cursors.primary_position(),
            (target.logical_line, target.start_col)
        );
    }

    #[test]
    fn test_paging_stops_at_the_last_visual_row() {
        let mut editor = wrapped_editor();
        let visual_lines = editor.visual_lines_cached(editor.viewport_width);
        let last = visual_lines.len() - 1;

        let start = &visual_lines[last];
        editor.cursors.primary_mut().position =
            (start.logical_line, start.start_col);
        editor.page_down();

        assert_eq!(editor.cursors.primary_position().0, start.logical_line);
    }

    #[test]
    fn test_rows_per_page_counts_whole_rows_only() {
        let mut editor = CodeEditor::new("line0\nline1", "py");

        editor.viewport_height = editor.line_height * 3.0;
        assert_eq!(editor.rows_per_page(), 3);

        // Half a row at the bottom is not a row the user can page onto.
        editor.viewport_height = editor.line_height * 3.5;
        assert_eq!(editor.rows_per_page(), 3);
    }

    #[test]
    fn test_column_on_visual_row_keeps_the_offset_inside_a_wrapped_line() {
        let buffer = TextBuffer::new("hello world");
        let current = VisualLine::new(0, 0, 0, 5);
        let target = VisualLine::new(0, 1, 5, 11);

        // Column 2 is the third character of the current row; it stays the
        // third character of the target row.
        assert_eq!(column_on_visual_row((0, 2), &current, &target, &buffer), 7);
    }

    #[test]
    fn test_column_on_visual_row_clamps_when_the_target_line_is_shorter() {
        let buffer = TextBuffer::new("longer line\nab");
        let current = VisualLine::new(0, 0, 0, 11);
        let target = VisualLine::new(1, 0, 0, 2);

        assert_eq!(column_on_visual_row((0, 9), &current, &target, &buffer), 2);
    }

    #[test]
    fn test_page_up_at_start() {
        let content = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut editor = CodeEditor::new(&content, "py");

        // Already at start
        editor.cursors.primary_mut().position = (0, 0);
        editor.page_up();
        assert_eq!(editor.cursors.primary_position().0, 0);
    }

    #[test]
    fn test_cursor_click_cjk() {
        use iced::Point;
        let mut editor = CodeEditor::new("你好", "txt");
        editor.set_line_numbers_enabled(false);
        // Disable folding so the gutter (line numbers + fold margin) is
        // zero-width; otherwise the fold margin shifts click coordinates.
        editor.set_folding_enabled(false);

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

        assert_eq!(editor.cursors.primary_position(), (0, 0));

        // Case 2: Click inside "你", at more than half its width.
        // Expect col 1
        editor
            .handle_mouse_click(Point::new((half_width + 2.0) + padding, 10.0));
        assert_eq!(editor.cursors.primary_position(), (0, 1));

        // Case 3: Click inside "好", at less than half its width.
        // "好" starts at full_char_width. Offset into "好" is < half_width.
        // Expect col 1 (start of "好")
        editor.handle_mouse_click(Point::new(
            (full_char_width + half_width - 2.0) + padding,
            10.0,
        ));
        assert_eq!(editor.cursors.primary_position(), (0, 1));

        // Case 4: Click inside "好", at more than half its width.
        // "好" starts at full_char_width. Offset into "好" is > half_width.
        // Expect col 2 (end of "好")
        editor.handle_mouse_click(Point::new(
            (full_char_width + half_width + 2.0) + padding,
            10.0,
        ));
        assert_eq!(editor.cursors.primary_position(), (0, 2));
    }

    #[test]
    fn test_multi_cursor_move_left() {
        let mut editor = CodeEditor::new("abc\ndef", "rs");
        editor.cursors.primary_mut().position = (0, 2);
        editor.cursors.add_cursor((1, 2));

        editor.move_cursor(ArrowDirection::Left);

        // Both cursors should have moved left by one
        let positions: Vec<(usize, usize)> =
            editor.cursors.iter().map(|c| c.position).collect();
        assert!(positions.contains(&(0, 1)));
        assert!(positions.contains(&(1, 1)));
    }

    #[test]
    fn test_multi_cursor_move_right() {
        let mut editor = CodeEditor::new("abc\ndef", "rs");
        editor.cursors.primary_mut().position = (0, 1);
        editor.cursors.add_cursor((1, 1));

        editor.move_cursor(ArrowDirection::Right);

        let positions: Vec<(usize, usize)> =
            editor.cursors.iter().map(|c| c.position).collect();
        assert!(positions.contains(&(0, 2)));
        assert!(positions.contains(&(1, 2)));
    }

    #[test]
    fn test_multi_cursor_move_deduplicates() {
        let mut editor = CodeEditor::new("abc", "rs");
        // Place two cursors adjacent, moving right will merge them
        editor.cursors.primary_mut().position = (0, 0);
        editor.cursors.add_cursor((0, 1));
        assert_eq!(editor.cursors.len(), 2);

        editor.move_cursor(ArrowDirection::Right);

        // Both moved right: (0,1) and (0,2). Still 2 distinct positions.
        assert_eq!(editor.cursors.len(), 2);
    }
}
