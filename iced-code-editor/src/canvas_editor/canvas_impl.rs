//! Canvas rendering implementation using Iced's `canvas::Program`.

use iced::advanced::input_method;
use iced::mouse;
use iced::widget::canvas::{self, Geometry};
use iced::{Color, Event, Point, Rectangle, Size, Theme, keyboard};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;

fn is_cursor_in_bounds(cursor: &mouse::Cursor, bounds: Rectangle) -> bool {
    match cursor {
        mouse::Cursor::Available(point) => bounds.contains(*point),
        mouse::Cursor::Levitating(point) => bounds.contains(*point),
        mouse::Cursor::Unavailable => false,
    }
}

/// Computes geometry (x start and width) for a text segment used in rendering or highlighting.
///
/// # Arguments
///
/// * `line_content`: full text content of the current line.
/// * `visual_start_col`: start column index of the current visual line.
/// * `segment_start_col`: start column index of the target segment (e.g. highlight).
/// * `segment_end_col`: end column index of the target segment.
/// * `base_offset`: base X offset (usually gutter_width + padding).
///
/// # Returns
///
/// x_start, width
///
/// # Remark
///
/// This function handles CJK character widths correctly to keep highlights accurate.
fn calculate_segment_geometry(
    line_content: &str,
    visual_start_col: usize,
    segment_start_col: usize,
    segment_end_col: usize,
    base_offset: f32,
    full_char_width: f32,
    char_width: f32,
) -> (f32, f32) {
    // Calculate prefix width relative to visual line start
    let prefix_len = segment_start_col.saturating_sub(visual_start_col);
    let prefix_text: String =
        line_content.chars().skip(visual_start_col).take(prefix_len).collect();
    let prefix_width =
        measure_text_width(&prefix_text, full_char_width, char_width);

    // Calculate segment width
    let segment_len = segment_end_col.saturating_sub(segment_start_col);
    let segment_text: String = line_content
        .chars()
        .skip(segment_start_col)
        .take(segment_len)
        .collect();
    let segment_width =
        measure_text_width(&segment_text, full_char_width, char_width);

    (base_offset + prefix_width, segment_width)
}

use super::wrapping::WrappingCalculator;
use super::{ArrowDirection, CodeEditor, Message, measure_text_width};
use iced::widget::canvas::Action;

impl canvas::Program<Message> for CodeEditor {
    type State = ();

    /// Renders the code editor's visual elements on the canvas, including text layout, syntax highlighting,
    /// cursor positioning, and other graphical aspects.
    ///
    /// # Arguments
    ///
    /// * `state` - The current state of the canvas
    /// * `renderer` - The renderer used for drawing
    /// * `theme` - The theme for styling
    /// * `bounds` - The rectangle bounds of the canvas
    /// * `cursor` - The mouse cursor position
    ///
    /// # Returns
    ///
    /// A vector of `Geometry` objects representing the drawn elements
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            // Initialize wrapping calculator
            let wrapping_calc = WrappingCalculator::new(
                self.wrap_enabled,
                self.wrap_column,
                self.full_char_width,
                self.char_width,
            );
            let visual_lines = wrapping_calc.calculate_visual_lines(
                &self.buffer,
                bounds.width,
                self.gutter_width(),
            );

            // Calculate visible line range based on viewport for optimized rendering
            // Use bounds.height as fallback when viewport_height is not yet initialized
            let effective_viewport_height = if self.viewport_height > 0.0 {
                self.viewport_height
            } else {
                bounds.height
            };
            let first_visible_line =
                (self.viewport_scroll / self.line_height).floor() as usize;
            let visible_lines_count =
                (effective_viewport_height / self.line_height).ceil() as usize
                    + 2;
            let last_visible_line = (first_visible_line + visible_lines_count)
                .min(visual_lines.len());

            // Load syntax highlighting
            let syntax_set = SyntaxSet::load_defaults_newlines();
            let theme_set = ThemeSet::load_defaults();
            let syntax_theme = &theme_set.themes["base16-ocean.dark"];

            let syntax_ref = match self.syntax.as_str() {
                "py" | "python" => syntax_set.find_syntax_by_extension("py"),
                "lua" => syntax_set.find_syntax_by_extension("lua"),
                "rs" | "rust" => syntax_set.find_syntax_by_extension("rs"),
                "js" | "javascript" => {
                    syntax_set.find_syntax_by_extension("js")
                }
                "html" | "htm" => syntax_set.find_syntax_by_extension("html"),
                "xml" | "svg" => syntax_set.find_syntax_by_extension("xml"),
                "css" => syntax_set.find_syntax_by_extension("css"),
                "json" => syntax_set.find_syntax_by_extension("json"),
                "md" | "markdown" => syntax_set.find_syntax_by_extension("md"),
                _ => Some(syntax_set.find_syntax_plain_text()),
            };

            // Draw only visible lines (virtual scrolling optimization)
            for (idx, visual_line) in visual_lines
                .iter()
                .enumerate()
                .skip(first_visible_line)
                .take(last_visible_line - first_visible_line)
            {
                let y = idx as f32 * self.line_height;

                // Note: Gutter background is handled by a container in view.rs
                // to ensure proper clipping when the pane is resized.

                // Draw line number only for first segment
                if self.line_numbers_enabled {
                    if visual_line.is_first_segment() {
                        let line_num = visual_line.logical_line + 1;
                        let line_num_text = format!("{}", line_num);
                        // Calculate actual text width and center in gutter
                        let text_width = measure_text_width(
                            &line_num_text,
                            self.full_char_width,
                            self.char_width,
                        );
                        let x_pos = (self.gutter_width() - text_width) / 2.0;
                        frame.fill_text(canvas::Text {
                            content: line_num_text,
                            position: Point::new(x_pos, y + 2.0),
                            color: self.style.line_number_color,
                            size: self.font_size.into(),
                            font: self.font,
                            ..canvas::Text::default()
                        });
                    } else {
                        // Draw wrap indicator for continuation lines
                        frame.fill_text(canvas::Text {
                            content: "↪".to_string(),
                            position: Point::new(
                                self.gutter_width() - 20.0,
                                y + 2.0,
                            ),
                            color: self.style.line_number_color,
                            size: self.font_size.into(),
                            font: self.font,
                            ..canvas::Text::default()
                        });
                    }
                }

                // Highlight current line (based on logical line)
                if visual_line.logical_line == self.cursor.0 {
                    frame.fill_rectangle(
                        Point::new(self.gutter_width(), y),
                        Size::new(
                            bounds.width - self.gutter_width(),
                            self.line_height,
                        ),
                        self.style.current_line_highlight,
                    );
                }

                // Draw text content with syntax highlighting
                let full_line_content =
                    self.buffer.line(visual_line.logical_line);

                // Convert character indices to byte indices for UTF-8 string slicing
                let start_byte = full_line_content
                    .char_indices()
                    .nth(visual_line.start_col)
                    .map_or(full_line_content.len(), |(idx, _)| idx);
                let end_byte = full_line_content
                    .char_indices()
                    .nth(visual_line.end_col)
                    .map_or(full_line_content.len(), |(idx, _)| idx);
                let line_segment = &full_line_content[start_byte..end_byte];

                if let Some(syntax) = syntax_ref {
                    let mut highlighter =
                        HighlightLines::new(syntax, syntax_theme);

                    // Highlight the full line to get correct token colors
                    let full_line_ranges = highlighter
                        .highlight_line(full_line_content, &syntax_set)
                        .unwrap_or_else(|_| {
                            vec![(Style::default(), full_line_content)]
                        });

                    // Extract only the ranges that fall within our segment
                    let mut x_offset = self.gutter_width() + 5.0;
                    let mut char_pos = 0;

                    for (style, text) in full_line_ranges {
                        let text_len = text.chars().count();
                        let text_end = char_pos + text_len;

                        // Check if this token intersects with our segment
                        if text_end > visual_line.start_col
                            && char_pos < visual_line.end_col
                        {
                            // Calculate the intersection
                            let segment_start =
                                char_pos.max(visual_line.start_col);
                            let segment_end = text_end.min(visual_line.end_col);

                            let text_start_offset =
                                segment_start.saturating_sub(char_pos);
                            let text_end_offset = text_start_offset
                                + (segment_end - segment_start);

                            // Convert character offsets to byte offsets for UTF-8 slicing
                            let start_byte = text
                                .char_indices()
                                .nth(text_start_offset)
                                .map_or(text.len(), |(idx, _)| idx);
                            let end_byte = text
                                .char_indices()
                                .nth(text_end_offset)
                                .map_or(text.len(), |(idx, _)| idx);

                            let segment_text = &text[start_byte..end_byte];

                            let color = Color::from_rgb(
                                f32::from(style.foreground.r) / 255.0,
                                f32::from(style.foreground.g) / 255.0,
                                f32::from(style.foreground.b) / 255.0,
                            );

                            frame.fill_text(canvas::Text {
                                content: segment_text.to_string(),
                                position: Point::new(x_offset, y + 2.0),
                                color,
                                size: self.font_size.into(),
                                font: self.font,
                                ..canvas::Text::default()
                            });

                            x_offset += measure_text_width(
                                segment_text,
                                self.full_char_width,
                                self.char_width,
                            );
                        }

                        char_pos = text_end;
                    }
                } else {
                    // Fallback to plain text
                    frame.fill_text(canvas::Text {
                        content: line_segment.to_string(),
                        position: Point::new(
                            self.gutter_width() + 5.0,
                            y + 2.0,
                        ),
                        color: self.style.text_color,
                        size: self.font_size.into(),
                        font: self.font,
                        ..canvas::Text::default()
                    });
                }
            }

            // Draw search match highlights
            if self.search_state.is_open && !self.search_state.query.is_empty()
            {
                let query_len = self.search_state.query.chars().count();

                for (match_idx, search_match) in
                    self.search_state.matches.iter().enumerate()
                {
                    // Determine if this is the current match
                    let is_current = self.search_state.current_match_index
                        == Some(match_idx);

                    let highlight_color = if is_current {
                        // Orange for current match
                        Color { r: 1.0, g: 0.6, b: 0.0, a: 0.4 }
                    } else {
                        // Yellow for other matches
                        Color { r: 1.0, g: 1.0, b: 0.0, a: 0.3 }
                    };

                    // Convert logical position to visual line
                    let start_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        search_match.line,
                        search_match.col,
                    );
                    let end_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        search_match.line,
                        search_match.col + query_len,
                    );

                    if let (Some(start_v), Some(end_v)) =
                        (start_visual, end_visual)
                    {
                        if start_v == end_v {
                            // Match within same visual line
                            let y = start_v as f32 * self.line_height;
                            let vl = &visual_lines[start_v];
                            let line_content =
                                self.buffer.line(vl.logical_line);

                            // Use calculate_segment_geometry to compute match position and width
                            let (x_start, match_width) =
                                calculate_segment_geometry(
                                    line_content,
                                    vl.start_col,
                                    search_match.col,
                                    search_match.col + query_len,
                                    self.gutter_width() + 5.0,
                                    self.full_char_width,
                                    self.char_width,
                                );
                            let x_end = x_start + match_width;

                            frame.fill_rectangle(
                                Point::new(x_start, y + 2.0),
                                Size::new(
                                    x_end - x_start,
                                    self.line_height - 4.0,
                                ),
                                highlight_color,
                            );
                        } else {
                            // Match spans multiple visual lines
                            for (v_idx, vl) in visual_lines
                                .iter()
                                .enumerate()
                                .skip(start_v)
                                .take(end_v - start_v + 1)
                            {
                                let y = v_idx as f32 * self.line_height;

                                let match_start_col = search_match.col;
                                let match_end_col =
                                    search_match.col + query_len;

                                let sel_start_col = if v_idx == start_v {
                                    match_start_col
                                } else {
                                    vl.start_col
                                };
                                let sel_end_col = if v_idx == end_v {
                                    match_end_col
                                } else {
                                    vl.end_col
                                };

                                let line_content =
                                    self.buffer.line(vl.logical_line);

                                let (x_start, sel_width) =
                                    calculate_segment_geometry(
                                        line_content,
                                        vl.start_col,
                                        sel_start_col,
                                        sel_end_col,
                                        self.gutter_width() + 5.0,
                                        self.full_char_width,
                                        self.char_width,
                                    );
                                let x_end = x_start + sel_width;

                                frame.fill_rectangle(
                                    Point::new(x_start, y + 2.0),
                                    Size::new(
                                        x_end - x_start,
                                        self.line_height - 4.0,
                                    ),
                                    highlight_color,
                                );
                            }
                        }
                    }
                }
            }

            // Draw selection highlight
            if let Some((start, end)) = self.get_selection_range()
                && start != end
            {
                let selection_color = Color { r: 0.3, g: 0.5, b: 0.8, a: 0.3 };

                if start.0 == end.0 {
                    // Single line selection - need to handle wrapped segments
                    let start_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        start.0,
                        start.1,
                    );
                    let end_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        end.0,
                        end.1,
                    );

                    if let (Some(start_v), Some(end_v)) =
                        (start_visual, end_visual)
                    {
                        if start_v == end_v {
                            // Selection within same visual line
                            let y = start_v as f32 * self.line_height;
                            let vl = &visual_lines[start_v];
                            let line_content =
                                self.buffer.line(vl.logical_line);

                            let (x_start, sel_width) =
                                calculate_segment_geometry(
                                    line_content,
                                    vl.start_col,
                                    start.1,
                                    end.1,
                                    self.gutter_width() + 5.0,
                                    self.full_char_width,
                                    self.char_width,
                                );
                            let x_end = x_start + sel_width;

                            frame.fill_rectangle(
                                Point::new(x_start, y + 2.0),
                                Size::new(
                                    x_end - x_start,
                                    self.line_height - 4.0,
                                ),
                                selection_color,
                            );
                        } else {
                            // Selection spans multiple visual lines (same logical line)
                            for (v_idx, vl) in visual_lines
                                .iter()
                                .enumerate()
                                .skip(start_v)
                                .take(end_v - start_v + 1)
                            {
                                let y = v_idx as f32 * self.line_height;

                                let sel_start_col = if v_idx == start_v {
                                    start.1
                                } else {
                                    vl.start_col
                                };
                                let sel_end_col = if v_idx == end_v {
                                    end.1
                                } else {
                                    vl.end_col
                                };

                                let line_content =
                                    self.buffer.line(vl.logical_line);

                                let (x_start, sel_width) =
                                    calculate_segment_geometry(
                                        line_content,
                                        vl.start_col,
                                        sel_start_col,
                                        sel_end_col,
                                        self.gutter_width() + 5.0,
                                        self.full_char_width,
                                        self.char_width,
                                    );
                                let x_end = x_start + sel_width;

                                frame.fill_rectangle(
                                    Point::new(x_start, y + 2.0),
                                    Size::new(
                                        x_end - x_start,
                                        self.line_height - 4.0,
                                    ),
                                    selection_color,
                                );
                            }
                        }
                    }
                } else {
                    // Multi-line selection
                    let start_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        start.0,
                        start.1,
                    );
                    let end_visual = WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        end.0,
                        end.1,
                    );

                    if let (Some(start_v), Some(end_v)) =
                        (start_visual, end_visual)
                    {
                        for (v_idx, vl) in visual_lines
                            .iter()
                            .enumerate()
                            .skip(start_v)
                            .take(end_v - start_v + 1)
                        {
                            let y = v_idx as f32 * self.line_height;

                            let sel_start_col = if vl.logical_line == start.0
                                && v_idx == start_v
                            {
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

                            let line_content =
                                self.buffer.line(vl.logical_line);

                            let (x_start, sel_width) =
                                calculate_segment_geometry(
                                    line_content,
                                    vl.start_col,
                                    sel_start_col,
                                    sel_end_col,
                                    self.gutter_width() + 5.0,
                                    self.full_char_width,
                                    self.char_width,
                                );
                            let x_end = x_start + sel_width;

                            frame.fill_rectangle(
                                Point::new(x_start, y + 2.0),
                                Size::new(
                                    x_end - x_start,
                                    self.line_height - 4.0,
                                ),
                                selection_color,
                            );
                        }
                    }
                }
            }

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
                && self.is_focused()
                && self.has_canvas_focus
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
                if let Some(cursor_visual) =
                    WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        self.cursor.0,
                        self.cursor.1,
                    )
                {
                    let vl = &visual_lines[cursor_visual];
                    let line_content = self.buffer.line(vl.logical_line);

                    // Compute the preedit region start X
                    // Use calculate_segment_geometry to ensure correct CJK width handling
                    let (cursor_x, _) = calculate_segment_geometry(
                        line_content,
                        vl.start_col,
                        self.cursor.1,
                        self.cursor.1,
                        self.gutter_width() + 5.0,
                        self.full_char_width,
                        self.char_width,
                    );
                    let cursor_y = cursor_visual as f32 * self.line_height;

                    if let Some(preedit) = self.ime_preedit.as_ref() {
                        let preedit_width = measure_text_width(
                            &preedit.content,
                            self.full_char_width,
                            self.char_width,
                        );

                        // 1. Draw preedit background (light translucent)
                        // This indicates the text is not committed yet
                        frame.fill_rectangle(
                            Point::new(cursor_x, cursor_y + 2.0),
                            Size::new(preedit_width, self.line_height - 4.0),
                            Color { r: 1.0, g: 1.0, b: 1.0, a: 0.08 },
                        );

                        // 2. Draw preedit selection (if any)
                        // IME may mark a selection inside preedit text (e.g. segmentation)
                        // The range uses UTF-8 byte indices, so slices must be safe
                        if let Some(range) = preedit.selection.as_ref()
                            && range.start != range.end
                        {
                            // Validate indices before slicing to prevent panic
                            if let Some((start, end)) =
                                validate_selection_indices(
                                    &preedit.content,
                                    range.start,
                                    range.end,
                                )
                            {
                                let selected_prefix = &preedit.content[..start];
                                let selected_text =
                                    &preedit.content[start..end];

                                let selection_x = cursor_x
                                    + measure_text_width(
                                        selected_prefix,
                                        self.full_char_width,
                                        self.char_width,
                                    );
                                let selection_w = measure_text_width(
                                    selected_text,
                                    self.full_char_width,
                                    self.char_width,
                                );

                                frame.fill_rectangle(
                                    Point::new(selection_x, cursor_y + 2.0),
                                    Size::new(
                                        selection_w,
                                        self.line_height - 4.0,
                                    ),
                                    Color { r: 0.3, g: 0.5, b: 0.8, a: 0.3 },
                                );
                            }
                        }

                        // 3. Draw preedit text itself
                        frame.fill_text(canvas::Text {
                            content: preedit.content.clone(),
                            position: Point::new(cursor_x, cursor_y + 2.0),
                            color: self.style.text_color,
                            size: self.font_size.into(),
                            font: self.font,
                            ..canvas::Text::default()
                        });

                        // 4. Draw bottom underline (IME state indicator)
                        frame.fill_rectangle(
                            Point::new(
                                cursor_x,
                                cursor_y + self.line_height - 3.0,
                            ),
                            Size::new(preedit_width, 1.0),
                            self.style.text_color,
                        );

                        // 5. Draw preedit caret
                        // If IME provides a caret position (usually selection end), draw a thin bar
                        if let Some(range) = preedit.selection.as_ref() {
                            let caret_end =
                                range.end.min(preedit.content.len());

                            // Validate caret position to avoid panic on invalid UTF-8 boundary
                            if caret_end <= preedit.content.len()
                                && preedit.content.is_char_boundary(caret_end)
                            {
                                let caret_prefix =
                                    &preedit.content[..caret_end];
                                let caret_x = cursor_x
                                    + measure_text_width(
                                        caret_prefix,
                                        self.full_char_width,
                                        self.char_width,
                                    );

                                frame.fill_rectangle(
                                    Point::new(caret_x, cursor_y + 2.0),
                                    Size::new(2.0, self.line_height - 4.0),
                                    self.style.text_color,
                                );
                            }
                        }
                    }
                }
            } else if self.show_cursor
                && self.cursor_visible
                && self.is_focused()
                && self.has_canvas_focus
            {
                // [Branch B] Normal caret rendering mode
                // ---------------------------------------------------------------------
                // When there is no IME preedit, draw the standard editor caret.
                // Key checks:
                // - is_focused(): the widget has Iced focus
                // - has_canvas_focus: internal focus state (mouse clicks, etc.)
                // - draw only when both are true to avoid ghost cursors
                // ---------------------------------------------------------------------

                // Map logical cursor position (Line, Col) to visual line index
                // to handle line wrapping changes
                if let Some(cursor_visual) =
                    WrappingCalculator::logical_to_visual(
                        &visual_lines,
                        self.cursor.0,
                        self.cursor.1,
                    )
                {
                    let vl = &visual_lines[cursor_visual];
                    let line_content = self.buffer.line(vl.logical_line);

                    // Compute exact caret X position
                    // Account for gutter width, left padding, and rendered prefix width
                    let (cursor_x, _) = calculate_segment_geometry(
                        line_content,
                        vl.start_col,
                        self.cursor.1,
                        self.cursor.1,
                        self.gutter_width() + 5.0,
                        self.full_char_width,
                        self.char_width,
                    );
                    let cursor_y = cursor_visual as f32 * self.line_height;

                    // Draw standard caret (2px vertical bar)
                    frame.fill_rectangle(
                        Point::new(cursor_x, cursor_y + 2.0),
                        Size::new(2.0, self.line_height - 4.0),
                        self.style.text_color,
                    );
                }
            }
        });

        vec![geometry]
    }

    /// Handles Canvas trait events, specifically keyboard input events and focus management for the code editor widget.
    ///
    /// # Arguments
    ///
    /// * `_state` - The mutable state of the canvas (unused in this implementation)
    /// * `event` - The input event to handle, such as keyboard presses
    /// * `bounds` - The rectangle bounds of the canvas widget
    /// * `cursor` - The current mouse cursor position and status
    ///
    /// # Returns
    ///
    /// An optional `Action<Message>` to perform, such as sending a message or redrawing the canvas
    fn update(
        &self,
        _state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                modifiers,
                text,
                ..
            }) => {
                // Only process keyboard events if this editor has focus
                let focused_id = super::FOCUSED_EDITOR_ID
                    .load(std::sync::atomic::Ordering::Relaxed);
                if focused_id != self.editor_id {
                    return None;
                }

                // Cursor outside canvas bounds
                if !is_cursor_in_bounds(&cursor, bounds) {
                    return None;
                }

                // Only process keyboard events if canvas has focus
                if !self.has_canvas_focus {
                    return None;
                }

                if self.ime_preedit.is_some()
                    && !(modifiers.control() || modifiers.command())
                {
                    return None;
                }

                // Handle Ctrl+C / Ctrl+Insert (copy)
                if (modifiers.control()
                    && matches!(key, keyboard::Key::Character(c) if c.as_str() == "c"))
                    || (modifiers.control()
                        && matches!(
                            key,
                            keyboard::Key::Named(keyboard::key::Named::Insert)
                        ))
                {
                    return Some(Action::publish(Message::Copy).and_capture());
                }

                // Handle Ctrl+Z (undo)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(z) if z.as_str() == "z")
                {
                    return Some(Action::publish(Message::Undo).and_capture());
                }

                // Handle Ctrl+Y (redo)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(y) if y.as_str() == "y")
                {
                    return Some(Action::publish(Message::Redo).and_capture());
                }

                // Handle Ctrl+F (open search)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(f) if f.as_str() == "f")
                    && self.search_replace_enabled
                {
                    return Some(
                        Action::publish(Message::OpenSearch).and_capture(),
                    );
                }

                // Handle Ctrl+H (open search and replace)
                if modifiers.control()
                    && matches!(key, keyboard::Key::Character(h) if h.as_str() == "h")
                    && self.search_replace_enabled
                {
                    return Some(
                        Action::publish(Message::OpenSearchReplace)
                            .and_capture(),
                    );
                }

                // Handle Escape (close search dialog if open)
                if matches!(
                    key,
                    keyboard::Key::Named(keyboard::key::Named::Escape)
                ) {
                    return Some(
                        Action::publish(Message::CloseSearch).and_capture(),
                    );
                }

                // Handle Tab (cycle forward in search dialog if open)
                if matches!(
                    key,
                    keyboard::Key::Named(keyboard::key::Named::Tab)
                ) && self.search_state.is_open
                {
                    if modifiers.shift() {
                        // Shift+Tab: cycle backward
                        return Some(
                            Action::publish(Message::SearchDialogShiftTab)
                                .and_capture(),
                        );
                    } else {
                        // Tab: cycle forward
                        return Some(
                            Action::publish(Message::SearchDialogTab)
                                .and_capture(),
                        );
                    }
                }

                // Handle F3 (find next) and Shift+F3 (find previous)
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::F3))
                    && self.search_replace_enabled
                {
                    if modifiers.shift() {
                        return Some(
                            Action::publish(Message::FindPrevious)
                                .and_capture(),
                        );
                    } else {
                        return Some(
                            Action::publish(Message::FindNext).and_capture(),
                        );
                    }
                }

                // Handle Ctrl+V / Shift+Insert (paste) - read clipboard and send paste message
                if (modifiers.control()
                    && matches!(key, keyboard::Key::Character(v) if v.as_str() == "v"))
                    || (modifiers.shift()
                        && matches!(
                            key,
                            keyboard::Key::Named(keyboard::key::Named::Insert)
                        ))
                {
                    // Return an action that requests clipboard read
                    return Some(Action::publish(
                        Message::Paste(String::new()),
                    ));
                }

                // Handle Ctrl+Home (go to start of document)
                if modifiers.control()
                    && matches!(
                        key,
                        keyboard::Key::Named(keyboard::key::Named::Home)
                    )
                {
                    return Some(
                        Action::publish(Message::CtrlHome).and_capture(),
                    );
                }

                // Handle Ctrl+End (go to end of document)
                if modifiers.control()
                    && matches!(
                        key,
                        keyboard::Key::Named(keyboard::key::Named::End)
                    )
                {
                    return Some(
                        Action::publish(Message::CtrlEnd).and_capture(),
                    );
                }

                // Handle Shift+Delete (delete selection)
                if modifiers.shift()
                    && matches!(
                        key,
                        keyboard::Key::Named(keyboard::key::Named::Delete)
                    )
                {
                    return Some(
                        Action::publish(Message::DeleteSelection).and_capture(),
                    );
                }

                // PRIORITY 1: Check if 'text' field has valid printable character
                // This handles:
                // - Numpad keys with NumLock ON (key=Named(ArrowDown), text=Some("2"))
                // - Regular typing with shift, accents, international layouts
                if let Some(text_content) = text
                    && !text_content.is_empty()
                    && !modifiers.control()
                    && !modifiers.alt()
                {
                    // Check if it's a printable character (not a control character)
                    // This filters out Enter (\n), Tab (\t), Delete (U+007F), etc.
                    if let Some(first_char) = text_content.chars().next()
                        && !first_char.is_control()
                    {
                        return Some(
                            Action::publish(Message::CharacterInput(
                                first_char,
                            ))
                            .and_capture(),
                        );
                    }
                }

                // PRIORITY 2: Handle special named keys (navigation, editing)
                // These are only processed if text didn't contain a printable character
                let message = match key {
                    keyboard::Key::Named(keyboard::key::Named::Backspace) => {
                        Some(Message::Backspace)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Delete) => {
                        Some(Message::Delete)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        Some(Message::Enter)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Tab) => {
                        // Insert 4 spaces for Tab
                        Some(Message::Tab)
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Up,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Down,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Left,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                        Some(Message::ArrowKey(
                            ArrowDirection::Right,
                            modifiers.shift(),
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageUp) => {
                        Some(Message::PageUp)
                    }
                    keyboard::Key::Named(keyboard::key::Named::PageDown) => {
                        Some(Message::PageDown)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Home) => {
                        Some(Message::Home(modifiers.shift()))
                    }
                    keyboard::Key::Named(keyboard::key::Named::End) => {
                        Some(Message::End(modifiers.shift()))
                    }
                    // PRIORITY 3: Fallback to extracting from 'key' if text was empty/control char
                    // This handles edge cases where text field is not populated
                    _ => {
                        if !modifiers.control()
                            && !modifiers.alt()
                            && let keyboard::Key::Character(c) = key
                            && !c.is_empty()
                        {
                            return c
                                .chars()
                                .next()
                                .map(Message::CharacterInput)
                                .map(|msg| Action::publish(msg).and_capture());
                        }
                        None
                    }
                };

                message.map(|msg| Action::publish(msg).and_capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                cursor.position_in(bounds).map(|position| {
                    // Don't capture the event so it can bubble up for focus management
                    Action::publish(Message::MouseClick(position))
                })
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                // Handle mouse drag for selection only when cursor is within bounds
                cursor.position_in(bounds).map(|position| {
                    Action::publish(Message::MouseDrag(position)).and_capture()
                })
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                // Only handle mouse release when cursor is within bounds
                // This prevents capturing events meant for other widgets
                if cursor.is_over(bounds) {
                    Some(Action::publish(Message::MouseRelease).and_capture())
                } else {
                    None
                }
            }
            Event::InputMethod(event) => {
                let focused_id = super::FOCUSED_EDITOR_ID
                    .load(std::sync::atomic::Ordering::Relaxed);
                if focused_id != self.editor_id {
                    return None;
                }

                if !is_cursor_in_bounds(&cursor, bounds) {
                    return None;
                }

                if !self.has_canvas_focus {
                    return None;
                }

                // IME event handling
                // ---------------------------------------------------------------------
                // Core mapping: convert Iced IME events into editor Messages
                //
                // Flow:
                // 1. Opened: IME activated (e.g. switching input method). Clear old preedit state.
                // 2. Preedit: User is composing (e.g. typing "nihao" before commit).
                //    - content: current candidate text
                //    - selection: selection range within the text, in bytes
                // 3. Commit: User confirms a candidate and commits text into the buffer.
                // 4. Closed: IME closed or lost focus.
                //
                // Safety checks:
                // - handle only when `focused_id` matches this editor ID
                // - handle only when `has_canvas_focus` is true
                // This ensures IME events are not delivered to the wrong widget.
                // ---------------------------------------------------------------------
                let message = match event {
                    input_method::Event::Opened => Message::ImeOpened,
                    input_method::Event::Preedit(content, selection) => {
                        Message::ImePreedit(content.clone(), selection.clone())
                    }
                    input_method::Event::Commit(content) => {
                        Message::ImeCommit(content.clone())
                    }
                    input_method::Event::Closed => Message::ImeClosed,
                };

                Some(Action::publish(message).and_capture())
            }
            _ => None,
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
    use super::*;
    use crate::canvas_editor::{CHAR_WIDTH, FONT_SIZE, compare_floats};
    use std::cmp::Ordering;

    #[test]
    fn test_calculate_segment_geometry_ascii() {
        // "Hello World"
        // "Hello " (6 chars) -> prefix
        // "World" (5 chars) -> segment
        // width("Hello ") = 6 * CHAR_WIDTH
        // width("World") = 5 * CHAR_WIDTH
        let content = "Hello World";
        let (x, w) = calculate_segment_geometry(
            content, 0, 6, 11, 0.0, FONT_SIZE, CHAR_WIDTH,
        );

        let expected_x = CHAR_WIDTH * 6.0;
        let expected_w = CHAR_WIDTH * 5.0;

        assert_eq!(
            compare_floats(x, expected_x),
            Ordering::Equal,
            "X position mismatch for ASCII"
        );
        assert_eq!(
            compare_floats(w, expected_w),
            Ordering::Equal,
            "Width mismatch for ASCII"
        );
    }

    #[test]
    fn test_calculate_segment_geometry_cjk() {
        // "你好世界"
        // "你好" (2 chars) -> prefix
        // "世界" (2 chars) -> segment
        // width("你好") = 2 * FONT_SIZE
        // width("世界") = 2 * FONT_SIZE
        let content = "你好世界";
        let (x, w) = calculate_segment_geometry(
            content, 0, 2, 4, 10.0, FONT_SIZE, CHAR_WIDTH,
        );

        let expected_x = 10.0 + FONT_SIZE * 2.0;
        let expected_w = FONT_SIZE * 2.0;

        assert_eq!(
            compare_floats(x, expected_x),
            Ordering::Equal,
            "X position mismatch for CJK"
        );
        assert_eq!(
            compare_floats(w, expected_w),
            Ordering::Equal,
            "Width mismatch for CJK"
        );
    }

    #[test]
    fn test_calculate_segment_geometry_mixed() {
        // "Hi你好"
        // "Hi" (2 chars) -> prefix
        // "你好" (2 chars) -> segment
        // width("Hi") = 2 * CHAR_WIDTH
        // width("你好") = 2 * FONT_SIZE
        let content = "Hi你好";
        let (x, w) = calculate_segment_geometry(
            content, 0, 2, 4, 0.0, FONT_SIZE, CHAR_WIDTH,
        );

        let expected_x = CHAR_WIDTH * 2.0;
        let expected_w = FONT_SIZE * 2.0;

        assert_eq!(
            compare_floats(x, expected_x),
            Ordering::Equal,
            "X position mismatch for mixed content"
        );
        assert_eq!(
            compare_floats(w, expected_w),
            Ordering::Equal,
            "Width mismatch for mixed content"
        );
    }

    #[test]
    fn test_calculate_segment_geometry_empty_range() {
        let content = "Hello";
        let (x, w) = calculate_segment_geometry(
            content, 0, 0, 0, 0.0, FONT_SIZE, CHAR_WIDTH,
        );
        assert!((x - 0.0).abs() < f32::EPSILON);
        assert!((w - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_calculate_segment_geometry_with_visual_offset() {
        // content: "0123456789"
        // visual_start_col: 2 (starts at '2')
        // segment: "34" (indices 3 to 5)
        // prefix: from visual start (2) to segment start (3) -> "2" (length 1)
        // prefix width: 1 * CHAR_WIDTH
        // segment width: 2 * CHAR_WIDTH
        let content = "0123456789";
        let (x, w) = calculate_segment_geometry(
            content, 2, 3, 5, 5.0, FONT_SIZE, CHAR_WIDTH,
        );

        let expected_x = 5.0 + CHAR_WIDTH * 1.0;
        let expected_w = CHAR_WIDTH * 2.0;

        assert_eq!(
            compare_floats(x, expected_x),
            Ordering::Equal,
            "X position mismatch with visual offset"
        );
        assert_eq!(
            compare_floats(w, expected_w),
            Ordering::Equal,
            "Width mismatch with visual offset"
        );
    }

    #[test]
    fn test_calculate_segment_geometry_out_of_bounds() {
        // Content length is 5 ("Hello")
        // Request start at 10, end at 15
        // visual_start 0
        // Prefix should consume whole string ("Hello") and stop.
        // Segment should be empty.
        let content = "Hello";
        let (x, w) = calculate_segment_geometry(
            content, 0, 10, 15, 0.0, FONT_SIZE, CHAR_WIDTH,
        );

        let expected_x = CHAR_WIDTH * 5.0; // Width of "Hello"
        let expected_w = 0.0;

        assert_eq!(
            compare_floats(x, expected_x),
            Ordering::Equal,
            "X position mismatch for out of bounds start"
        );
        assert!(
            (w - expected_w).abs() < f32::EPSILON,
            "Width should be 0 for out of bounds segment"
        );
    }

    #[test]
    fn test_calculate_segment_geometry_special_chars() {
        // Emoji "👋" (width > 1 => FONT_SIZE)
        // Tab "\t" (width None => 0.0)
        let content = "A👋\tB";
        // Measure "👋" (index 1 to 2)
        // Indices in chars: 'A' (0), '👋' (1), '\t' (2), 'B' (3)

        // Segment covering Emoji
        let (x, w) = calculate_segment_geometry(
            content, 0, 1, 2, 0.0, FONT_SIZE, CHAR_WIDTH,
        );
        let expected_x_emoji = CHAR_WIDTH; // 'A'
        let expected_w_emoji = FONT_SIZE; // '👋'

        assert_eq!(
            compare_floats(x, expected_x_emoji),
            Ordering::Equal,
            "X pos for emoji"
        );
        assert_eq!(
            compare_floats(w, expected_w_emoji),
            Ordering::Equal,
            "Width for emoji"
        );

        // Segment covering Tab
        let (x_tab, w_tab) = calculate_segment_geometry(
            content, 0, 2, 3, 0.0, FONT_SIZE, CHAR_WIDTH,
        );
        let expected_x_tab = CHAR_WIDTH + FONT_SIZE; // 'A' + '👋'
        let expected_w_tab = 0.0; // Tab width is 0 in this implementation

        assert_eq!(
            compare_floats(x_tab, expected_x_tab),
            Ordering::Equal,
            "X pos for tab"
        );
        assert_eq!(
            compare_floats(w_tab, expected_w_tab),
            Ordering::Equal,
            "Width for tab"
        );
    }

    #[test]
    fn test_calculate_segment_geometry_inverted_range() {
        // Start 5, End 3
        // Should result in empty segment at start 5
        let content = "0123456789";
        let (x, w) = calculate_segment_geometry(
            content, 0, 5, 3, 0.0, FONT_SIZE, CHAR_WIDTH,
        );

        let expected_x = CHAR_WIDTH * 5.0;
        let expected_w = 0.0;

        assert_eq!(
            compare_floats(x, expected_x),
            Ordering::Equal,
            "X pos for inverted range"
        );
        assert!(
            (w - expected_w).abs() < f32::EPSILON,
            "Width for inverted range"
        );
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
