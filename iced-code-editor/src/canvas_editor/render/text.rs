//! Text-layer drawing: placing glyphs, tab/whitespace expansion, indentation
//! guides, colour swatches and bracket-pair colorization on the canvas for
//! [`CodeEditor`].
//!
//! The colours themselves come from [`super::highlighting`]; this module only
//! decides where they land.

use iced::widget::canvas;
use iced::{Point, Size};
use std::borrow::Cow;
use syntect::parsing::SyntaxSet;

use crate::buffer::text_utils::char_range_to_byte_range;

use super::wrapping::VisualLine;
use crate::canvas_editor::IndentStyle;
use crate::canvas_editor::features::{
    bracket_match, color_preview, indent_guides,
};
use crate::canvas_editor::{
    CodeEditor, TAB_WIDTH, measure_char_width, measure_text_width,
};

/// Width in pixels of a single indentation guide line.
const INDENT_GUIDE_WIDTH: f32 = 1.0;

/// Side of a color-preview swatch, as a fraction of the line height.
const SWATCH_SIZE_RATIO: f32 = 0.6;

/// Horizontal gap in pixels between a color literal and its swatch.
const SWATCH_GAP: f32 = 3.0;

/// Thickness in pixels of the border framing a color-preview swatch.
const SWATCH_BORDER_WIDTH: f32 = 1.0;

/// Returns whether a literal's swatch belongs to the visual segment spanning
/// `[segment_start_col, segment_end_col)`.
///
/// The swatch is drawn just *after* the literal, so what decides which segment
/// owns it is where the literal **ends**, not where it starts. A literal
/// straddling a soft wrap therefore gets its swatch on the segment carrying its
/// last character, and gets exactly one: the two comparisons are the two halves
/// of a half-open interval, so no segment can claim the same swatch twice and
/// none can drop it.
///
/// # Arguments
///
/// * `literal_end_col` - Column one past the literal's last character
/// * `segment_start_col` - First column of the segment, inclusive
/// * `segment_end_col` - Column one past the segment's last character
///
/// # Returns
///
/// `true` when this segment is the one that must draw the swatch
fn swatch_belongs_to_segment(
    literal_end_col: usize,
    segment_start_col: usize,
    segment_end_col: usize,
) -> bool {
    literal_end_col > segment_start_col && literal_end_col <= segment_end_col
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
pub(super) fn calculate_segment_geometry(
    line_content: &str,
    visual_start_col: usize,
    segment_start_col: usize,
    segment_end_col: usize,
    base_offset: f32,
    full_char_width: f32,
    char_width: f32,
) -> (f32, f32) {
    // Clamp the segment to the current visual line so callers can safely pass
    // logical selection/match columns without worrying about wrapping boundaries.
    let segment_start_col = segment_start_col.max(visual_start_col);
    let segment_end_col = segment_end_col.max(segment_start_col);

    let mut prefix_width = 0.0;
    let mut segment_width = 0.0;

    // Compute widths directly from the source string to avoid allocating
    // intermediate `String` slices for prefix/segment.
    for (i, c) in line_content.chars().enumerate() {
        if i >= segment_end_col {
            break;
        }

        let w = measure_char_width(c, full_char_width, char_width);

        if i >= visual_start_col && i < segment_start_col {
            prefix_width += w;
        } else if i >= segment_start_col {
            segment_width += w;
        }
    }

    (base_offset + prefix_width, segment_width)
}

/// Replaces each tab with `tab_width` spaces, borrowing when there is nothing
/// to expand. Shared by the canvas text layer and the sticky-scroll headers so
/// both render indentation identically.
pub(crate) fn expand_tabs(text: &str, tab_width: usize) -> Cow<'_, str> {
    if !text.contains('\t') {
        return Cow::Borrowed(text);
    }

    let mut expanded = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch == '\t' {
            for _ in 0..tab_width {
                expanded.push(' ');
            }
        } else {
            expanded.push(ch);
        }
    }

    Cow::Owned(expanded)
}

/// Expands tabs and replaces whitespace with visible symbols: `\t` → `→` +
/// `·` fill, ` ` → `·`. The output has the same logical width as the
/// `expand_tabs` output, so existing width measurements remain valid.
pub(crate) fn expand_tabs_visible(text: &str, tab_width: usize) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    for ch in text.chars() {
        match ch {
            '\t' => {
                result.push('→');
                for _ in 1..tab_width {
                    result.push('·');
                }
            }
            ' ' => result.push('·'),
            other => result.push(other),
        }
    }
    result
}

/// Splits a string (already processed by [`expand_tabs_visible`]) into
/// alternating `(is_whitespace, segment)` pairs, where whitespace segments
/// consist exclusively of `·` and `→` characters.
fn split_whitespace_segments(text: &str) -> Vec<(bool, &str)> {
    if text.is_empty() {
        return vec![];
    }

    let mut result = Vec::new();
    let mut seg_start = 0usize;
    let mut chars = text.char_indices().peekable();

    let is_ws_char = |c: char| c == '·' || c == '→';

    let first_ch = chars.peek().map(|(_, c)| *c).unwrap_or(' ');
    let mut current_is_ws = is_ws_char(first_ch);

    for (byte_idx, ch) in chars {
        let ch_is_ws = is_ws_char(ch);
        if ch_is_ws != current_is_ws {
            result.push((current_is_ws, &text[seg_start..byte_idx]));
            seg_start = byte_idx;
            current_is_ws = ch_is_ws;
        }
    }
    result.push((current_is_ws, &text[seg_start..]));
    result
}
/// Context for canvas rendering operations.
///
/// This struct packages commonly used rendering parameters to reduce
/// method signature complexity and improve code maintainability.
pub(super) struct RenderContext<'a> {
    /// Visual lines calculated from wrapping
    pub(super) visual_lines: &'a [VisualLine],
    /// Width of the canvas bounds
    pub(super) bounds_width: f32,
    /// Width of the line number gutter
    pub(super) gutter_width: f32,
    /// Height of each line in pixels
    pub(super) line_height: f32,
    /// Font size in pixels
    pub(super) font_size: f32,
    /// Full character width for wide characters (e.g., CJK)
    pub(super) full_char_width: f32,
    /// Character width for narrow characters
    pub(super) char_width: f32,
    /// Font to use for rendering text
    pub(super) font: iced::Font,
    /// Horizontal scroll offset in pixels (subtracted from text X positions)
    pub(super) horizontal_scroll_offset: f32,
}

impl CodeEditor {
    /// Draws text content with syntax highlighting or plain text fallback.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_line` - The visual line to render
    /// * `y` - Y position for rendering
    /// * `syntax_ref` - Optional syntax reference for highlighting
    /// * `syntax_set` - Syntax set for highlighting
    /// * `syntax_theme` - Theme for syntax highlighting
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw_text_with_syntax_highlighting(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
        syntax_ref: Option<&syntect::parsing::SyntaxReference>,
        syntax_set: &SyntaxSet,
        syntax_theme: Option<&syntect::highlighting::Theme>,
    ) {
        if let (Some(syntax), Some(syntax_theme)) = (syntax_ref, syntax_theme) {
            // Reuse the memoized full-line spans; only the visible segment of
            // the (possibly wrapped) line is positioned and drawn here.
            let spans = self.highlighted_line_cached(
                visual_line.logical_line,
                syntax,
                syntax_theme,
                syntax_set,
            );

            let mut x_offset =
                ctx.gutter_width + 5.0 - ctx.horizontal_scroll_offset;
            let mut char_pos = 0;

            for (color, text) in spans.iter() {
                let text_len = text.chars().count();
                let text_end = char_pos + text_len;

                // Check if this token intersects with our segment
                if text_end > visual_line.start_col
                    && char_pos < visual_line.end_col
                {
                    // Calculate the intersection
                    let segment_start = char_pos.max(visual_line.start_col);
                    let segment_end = text_end.min(visual_line.end_col);

                    let text_start_offset =
                        segment_start.saturating_sub(char_pos);
                    let text_end_offset =
                        text_start_offset + (segment_end - segment_start);

                    let (start_byte, end_byte) = char_range_to_byte_range(
                        text,
                        text_start_offset,
                        text_end_offset,
                    );

                    let segment_text = &text[start_byte..end_byte];
                    let display_text = if self.show_whitespace {
                        expand_tabs_visible(segment_text, TAB_WIDTH)
                    } else {
                        expand_tabs(segment_text, TAB_WIDTH).into_owned()
                    };
                    let display_width = measure_text_width(
                        &display_text,
                        ctx.full_char_width,
                        ctx.char_width,
                    );

                    if self.show_whitespace {
                        let ws_color = self.style.whitespace_color;
                        let mut seg_x = x_offset;
                        for (is_ws, seg) in
                            split_whitespace_segments(&display_text)
                        {
                            let seg_color =
                                if is_ws { ws_color } else { *color };
                            let seg_width = measure_text_width(
                                seg,
                                ctx.full_char_width,
                                ctx.char_width,
                            );
                            frame.fill_text(canvas::Text {
                                content: seg.to_string(),
                                position: Point::new(seg_x, y + 2.0),
                                color: seg_color,
                                size: ctx.font_size.into(),
                                font: ctx.font,
                                ..canvas::Text::default()
                            });
                            seg_x += seg_width;
                        }
                    } else {
                        frame.fill_text(canvas::Text {
                            content: display_text,
                            position: Point::new(x_offset, y + 2.0),
                            color: *color,
                            size: ctx.font_size.into(),
                            font: ctx.font,
                            ..canvas::Text::default()
                        });
                    }

                    x_offset += display_width;
                }

                char_pos = text_end;
            }
        } else {
            // Fallback to plain text
            let full_line_content = self.buffer.line(visual_line.logical_line);
            let (start_byte, end_byte) = char_range_to_byte_range(
                full_line_content,
                visual_line.start_col,
                visual_line.end_col,
            );
            let line_segment = &full_line_content[start_byte..end_byte];
            let display_text = if self.show_whitespace {
                expand_tabs_visible(line_segment, TAB_WIDTH)
            } else {
                expand_tabs(line_segment, TAB_WIDTH).into_owned()
            };
            let base_x = ctx.gutter_width + 5.0 - ctx.horizontal_scroll_offset;
            if self.show_whitespace {
                let ws_color = self.style.whitespace_color;
                let text_color = self.style.text_color;
                let mut seg_x = base_x;
                for (is_ws, seg) in split_whitespace_segments(&display_text) {
                    let seg_color = if is_ws { ws_color } else { text_color };
                    let seg_width = measure_text_width(
                        seg,
                        ctx.full_char_width,
                        ctx.char_width,
                    );
                    frame.fill_text(canvas::Text {
                        content: seg.to_string(),
                        position: Point::new(seg_x, y + 2.0),
                        color: seg_color,
                        size: ctx.font_size.into(),
                        font: ctx.font,
                        ..canvas::Text::default()
                    });
                    seg_x += seg_width;
                }
            } else {
                frame.fill_text(canvas::Text {
                    content: display_text,
                    position: Point::new(base_x, y + 2.0),
                    color: self.style.text_color,
                    size: ctx.font_size.into(),
                    font: ctx.font,
                    ..canvas::Text::default()
                });
            }
        }
    }

    /// Draws the vertical indentation guides for `visual_line`.
    ///
    /// One thin vertical line is drawn per indentation level, at display
    /// columns `0`, `unit`, `2 * unit`, … where `unit` comes from
    /// [`CodeEditor::indent_style`]. The number of levels is decided by
    /// [`indent_guides::guide_levels`], which also gives blank lines the level
    /// of their surrounding block. No-op when the feature is disabled.
    ///
    /// Guides are skipped on wrapped continuation segments: every visual line
    /// starts drawing at the same base X, so a guide placed at its original
    /// column would sit on top of the wrapped text rather than in its
    /// indentation.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_line` - The visual line to render
    /// * `y` - Y position for rendering
    pub(super) fn draw_indent_guides(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
    ) {
        if !self.show_indent_guides || !visual_line.is_first_segment() {
            return;
        }

        let unit = match self.indent_style {
            IndentStyle::Spaces(width) => usize::from(width),
            IndentStyle::Tab => TAB_WIDTH,
        };
        let levels = indent_guides::guide_levels(
            &self.buffer,
            visual_line.logical_line,
            unit,
        );

        let base_x = ctx.gutter_width + 5.0 - ctx.horizontal_scroll_offset;
        for level in 0..levels {
            let x = base_x + (level * unit) as f32 * ctx.char_width;
            frame.fill_rectangle(
                Point::new(x, y),
                Size::new(INDENT_GUIDE_WIDTH, ctx.line_height),
                self.style.indent_guide_color,
            );
        }
    }

    /// Draws the inline color-preview swatches for `visual_line`.
    ///
    /// Every color literal found on the logical line gets a small square,
    /// filled with the color it denotes, drawn just after it.
    /// The square is framed so that a color close to the editor background
    /// stays visible, and translucent colors are drawn over a background-filled
    /// square so their opacity reads correctly. No-op when the feature is
    /// disabled.
    ///
    /// The swatch is plain geometry, and iced draws all text above all
    /// geometry, so the character following the literal stays readable even
    /// when the square extends under it.
    ///
    /// A literal split by soft wrapping is drawn once, on the segment holding
    /// its last character, which is where the swatch belongs. Every segment of
    /// a wrapped line therefore has to know the whole line's literals, which
    /// is what `literals` is for: it scans each logical line once and hands
    /// the result to all of that line's segments.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_line` - The visual line to render
    /// * `y` - Y position for rendering
    /// * `literals` - Per-draw-pass memo of the logical lines already scanned
    pub(super) fn draw_color_swatches(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
        literals: &mut color_preview::LineLiterals,
    ) {
        if !self.show_color_previews {
            return;
        }

        let line_content = self.buffer.line(visual_line.logical_line);
        let side = (ctx.line_height * SWATCH_SIZE_RATIO).floor().max(1.0);
        let inner_side = (side - 2.0 * SWATCH_BORDER_WIDTH).max(1.0);

        for literal in literals.get(&self.buffer, visual_line.logical_line) {
            if !swatch_belongs_to_segment(
                literal.end_col,
                visual_line.start_col,
                visual_line.end_col,
            ) {
                continue;
            }

            let (x, _width) = calculate_segment_geometry(
                line_content,
                visual_line.start_col,
                literal.end_col,
                literal.end_col,
                ctx.gutter_width + 5.0,
                ctx.full_char_width,
                ctx.char_width,
            );
            let border_position = Point::new(
                x - ctx.horizontal_scroll_offset + SWATCH_GAP,
                y + (ctx.line_height - side) / 2.0,
            );
            let inner_position = Point::new(
                border_position.x + SWATCH_BORDER_WIDTH,
                border_position.y + SWATCH_BORDER_WIDTH,
            );

            frame.fill_rectangle(
                border_position,
                Size::new(side, side),
                self.style.gutter_border,
            );
            for color in [self.style.background, literal.color] {
                frame.fill_rectangle(
                    inner_position,
                    Size::new(inner_side, inner_side),
                    color,
                );
            }
        }
    }

    /// Draws bracket-pair colorization (rainbow brackets) for `visual_line`.
    ///
    /// Each `( ) [ ] { }` character on the line is redrawn on top of the
    /// already-rendered syntax-highlighted text, colored by its nesting
    /// depth (see [`bracket_match::bracket_depth_indices`]) so a
    /// matching pair always shares the same color, cycling through the
    /// theme-matched palette returned by [`CodeEditor::bracket_pair_colors`]
    /// as depth increases. No-op when the feature is disabled.
    ///
    /// # Arguments
    ///
    /// * `frame` - The canvas frame to draw on
    /// * `ctx` - Rendering context containing visual lines and metrics
    /// * `visual_line` - The visual line to render
    /// * `y` - Y position for rendering
    pub(super) fn draw_bracket_pair_colors(
        &self,
        frame: &mut canvas::Frame,
        ctx: &RenderContext,
        visual_line: &VisualLine,
        y: f32,
    ) {
        if !self.bracket_pair_colorization_enabled {
            return;
        }

        let palette = self.bracket_pair_colors();
        let logical_line = visual_line.logical_line;
        let start_depth = self
            .bracket_depth_cache
            .borrow_mut()
            .depth_at_line_start(&self.buffer, logical_line);

        let line_content = self.buffer.line(logical_line);
        let indices =
            bracket_match::bracket_depth_indices(line_content, start_depth);

        for (col, depth) in indices {
            if col < visual_line.start_col || col >= visual_line.end_col {
                continue;
            }
            let Some(ch) = line_content.chars().nth(col) else {
                continue;
            };

            let (x, _width) = calculate_segment_geometry(
                line_content,
                visual_line.start_col,
                col,
                col + 1,
                ctx.gutter_width + 5.0,
                ctx.full_char_width,
                ctx.char_width,
            );
            frame.fill_text(canvas::Text {
                content: ch.to_string(),
                position: Point::new(x - ctx.horizontal_scroll_offset, y + 2.0),
                color: palette[depth % palette.len()],
                size: ctx.font_size.into(),
                font: ctx.font,
                ..canvas::Text::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;
    use crate::canvas_editor::{CHAR_WIDTH, FONT_SIZE, compare_floats};

    #[test]
    fn test_a_swatch_lands_on_the_segment_holding_the_literals_last_character()
    {
        // A literal spanning columns 30..37 on a line wrapped every 20
        // columns: only the second segment, which holds column 36, draws it.
        assert!(
            !swatch_belongs_to_segment(37, 0, 20),
            "the first segment ends before the literal does"
        );
        assert!(swatch_belongs_to_segment(37, 20, 40));
        assert!(
            !swatch_belongs_to_segment(37, 40, 60),
            "a later segment must not draw it a second time"
        );
    }

    #[test]
    fn test_every_column_is_claimed_by_exactly_one_segment() {
        // The property the two comparisons exist to guarantee: whatever the
        // literal's end column, exactly one segment of the wrapped line owns
        // its swatch -- never zero (a dropped swatch), never two (a doubled
        // one). This is what a rewrite of the filter would have to preserve.
        const SEGMENTS: [(usize, usize); 3] = [(0, 20), (20, 40), (40, 60)];

        for literal_end_col in 1..=60 {
            let owners = SEGMENTS
                .iter()
                .filter(|(start, end)| {
                    swatch_belongs_to_segment(literal_end_col, *start, *end)
                })
                .count();

            assert_eq!(owners, 1, "end column {literal_end_col} has {owners}");
        }
    }

    #[test]
    fn test_a_swatch_at_the_very_end_of_a_segment_belongs_to_it() {
        // The boundary the half-open interval turns on: a literal ending
        // exactly where a segment ends is drawn by that segment, not the next.
        assert!(swatch_belongs_to_segment(20, 0, 20));
        assert!(!swatch_belongs_to_segment(20, 20, 40));
    }

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
        // Tab "\t" (width = 4 * CHAR_WIDTH)
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
        let expected_w_tab =
            CHAR_WIDTH * crate::canvas_editor::TAB_WIDTH as f32;

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
    fn test_expand_tabs_visible_spaces() {
        assert_eq!(expand_tabs_visible("a b", 4), "a·b");
        assert_eq!(expand_tabs_visible("  x  ", 4), "··x··");
    }

    #[test]
    fn test_expand_tabs_visible_tabs() {
        // tab_width = 4: '\t' → '→' + 3 × '·'
        assert_eq!(expand_tabs_visible("\t", 4), "→···");
        assert_eq!(expand_tabs_visible("a\tb", 4), "a→···b");
    }

    #[test]
    fn test_expand_tabs_visible_no_whitespace() {
        assert_eq!(expand_tabs_visible("hello", 4), "hello");
    }

    #[test]
    fn test_split_whitespace_segments_mixed() {
        let segs = split_whitespace_segments("a·b");
        assert_eq!(segs, vec![(false, "a"), (true, "·"), (false, "b")]);
    }

    #[test]
    fn test_split_whitespace_segments_leading_ws() {
        let segs = split_whitespace_segments("··x");
        assert_eq!(segs, vec![(true, "··"), (false, "x")]);
    }

    #[test]
    fn test_split_whitespace_segments_all_ws() {
        let segs = split_whitespace_segments("···");
        assert_eq!(segs, vec![(true, "···")]);
    }

    #[test]
    fn test_split_whitespace_segments_empty() {
        let segs = split_whitespace_segments("");
        assert!(segs.is_empty());
    }
}
