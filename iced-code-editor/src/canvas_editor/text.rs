//! Text-layer rendering: syntax highlighting, tab/whitespace expansion, and
//! bracket-pair colorization for [`CodeEditor`].

use iced::widget::canvas;
use iced::{Color, Point};
use std::borrow::Cow;
use std::rc::Rc;
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    HighlightIterator, HighlightState, Highlighter, Style,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

use crate::text_utils::char_range_to_byte_range;

use super::wrapping::VisualLine;
use super::{CodeEditor, measure_text_width};

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

        let w = super::measure_char_width(c, full_char_width, char_width);

        if i >= visual_start_col && i < segment_start_col {
            prefix_width += w;
        } else if i >= segment_start_col {
            segment_width += w;
        }
    }

    (base_offset + prefix_width, segment_width)
}

fn expand_tabs(text: &str, tab_width: usize) -> Cow<'_, str> {
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
fn expand_tabs_visible(text: &str, tab_width: usize) -> String {
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

/// Converts a syntect highlight [`Style`] into an iced [`Color`].
///
/// Only the foreground color is used; alpha is left fully opaque.
///
/// # Arguments
///
/// * `style` - The syntect style whose foreground color is converted.
fn color_from_style(style: Style) -> Color {
    Color::from_rgb(
        f32::from(style.foreground.r) / 255.0,
        f32::from(style.foreground.g) / 255.0,
        f32::from(style.foreground.b) / 255.0,
    )
}

/// Tokenizes a full logical line into colored spans using syntect.
///
/// The returned spans cover the entire line in order, each pairing an iced
/// [`Color`] with the owned token text. Each call highlights the line
/// independently from the syntax's initial state, so it does not handle
/// multi-line constructs; it is used for tests and benchmarks. Rendering uses
/// the sequential [`CodeEditor::highlighted_line_cached`] instead.
///
/// # Arguments
///
/// * `line` - The full logical line content (without trailing newline).
/// * `syntax` - The syntect syntax definition to tokenize with.
/// * `theme` - The syntect highlighting theme providing token colors.
/// * `syntax_set` - The syntax set the `syntax` belongs to.
///
/// # Returns
///
/// The ordered colored spans covering the entire line.
pub fn highlight_line_spans(
    line: &str,
    syntax: &syntect::parsing::SyntaxReference,
    theme: &syntect::highlighting::Theme,
    syntax_set: &SyntaxSet,
) -> Vec<(Color, String)> {
    let mut highlighter = HighlightLines::new(syntax, theme);
    let ranges = highlighter
        .highlight_line(line, syntax_set)
        .unwrap_or_else(|_| vec![(Style::default(), line)]);

    ranges
        .into_iter()
        .map(|(style, text)| (color_from_style(style), text.to_string()))
        .collect()
}

/// Fixed color cycle for bracket-pair colorization, indexed by nesting depth
/// modulo its length so a matching pair always shares a color. Matches the
/// well-known VS Code default rainbow-bracket palette (gold, orchid, light
/// sky blue) for a look users are likely already familiar with.
const BRACKET_PAIR_COLORS: [Color; 3] = [
    Color { r: 1.0, g: 0.843, b: 0.0, a: 1.0 }, // gold
    Color { r: 0.855, g: 0.439, b: 0.839, a: 1.0 }, // orchid
    Color { r: 0.529, g: 0.808, b: 0.980, a: 1.0 }, // light sky blue
];

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
    /// Returns the memoized syntax-highlighted spans for a logical line.
    ///
    /// Highlighting is performed sequentially: lines `0..=logical_line` are
    /// tokenized in order, each resuming from the syntect state left by the
    /// previous line, so multi-line constructs (block comments, multi-line
    /// strings) are colored correctly. The result is stored as a dense valid
    /// prefix in [`HighlightCache`] and reused across wrapped visual segments
    /// and across renders; an edit truncates the prefix from the changed line
    /// (see [`CodeEditor::invalidate_highlight_from`]) instead of clearing it,
    /// so deep lines are not re-parsed from the top on every keystroke. The
    /// cache is reset only when the active syntax changes.
    ///
    /// # Arguments
    ///
    /// * `logical_line` - Index of the logical line in the buffer.
    /// * `syntax` - The syntect syntax definition to tokenize with.
    /// * `theme` - The syntect highlighting theme providing token colors.
    /// * `syntax_set` - The syntax set the `syntax` belongs to.
    ///
    /// # Returns
    ///
    /// A shared handle to the line's colored token spans.
    fn highlighted_line_cached(
        &self,
        logical_line: usize,
        syntax: &syntect::parsing::SyntaxReference,
        theme: &syntect::highlighting::Theme,
        syntax_set: &SyntaxSet,
    ) -> Rc<Vec<(Color, String)>> {
        let mut guard = self.highlight_cache.borrow_mut();

        // Reset the whole cache only when the active syntax changes.
        let needs_reset =
            guard.as_ref().is_none_or(|cache| cache.syntax() != self.syntax);
        if needs_reset {
            *guard = Some(super::HighlightCache::new(self.syntax.clone()));
        }

        let Some(cache) = guard.as_mut() else {
            // Unreachable: populated just above. `unwrap`/`panic` are denied,
            // so fall back to a single independent highlight without caching.
            return Rc::new(highlight_line_spans(
                self.buffer.line(logical_line),
                syntax,
                theme,
                syntax_set,
            ));
        };

        if let Some(spans) = cache.spans(logical_line) {
            return spans;
        }

        // Extend the valid prefix sequentially up to `logical_line`, carrying
        // the parser/highlight state forward across lines.
        let highlighter = Highlighter::new(theme);
        let (mut parse_state, mut highlight_state) =
            cache.resume_state().unwrap_or_else(|| {
                (
                    ParseState::new(syntax),
                    HighlightState::new(&highlighter, ScopeStack::new()),
                )
            });

        let line_count = self.buffer.line_count();
        let target = logical_line.min(line_count.saturating_sub(1));
        let missing_lines =
            target.saturating_add(1).saturating_sub(cache.valid_len());
        let lines_to_parse =
            missing_lines.min(self.highlight_lines_remaining.get());
        let parse_end = cache
            .valid_len()
            .saturating_add(lines_to_parse)
            .saturating_sub(1)
            .min(target);
        let mut result = None;
        if lines_to_parse > 0 {
            for index in cache.valid_len()..=parse_end {
                // syntect's `_newlines` syntaxes expect a trailing '\n' for correct
                // end-of-line context handling; the stored buffer line has none.
                let mut line = self.buffer.line(index).to_string();
                line.push('\n');

                let ops = parse_state
                    .parse_line(&line, syntax_set)
                    .unwrap_or_default();
                let spans: Vec<(Color, String)> = HighlightIterator::new(
                    &mut highlight_state,
                    &ops,
                    &line,
                    &highlighter,
                )
                .filter_map(|(style, text)| {
                    let text = text.strip_suffix('\n').unwrap_or(text);
                    if text.is_empty() {
                        None
                    } else {
                        Some((color_from_style(style), text.to_string()))
                    }
                })
                .collect();

                let spans = Rc::new(spans);
                cache.push_line(
                    Rc::clone(&spans),
                    parse_state.clone(),
                    highlight_state.clone(),
                );
                if index == logical_line {
                    result = Some(spans);
                }
            }
        }
        self.highlight_lines_remaining.set(
            self.highlight_lines_remaining.get().saturating_sub(lines_to_parse),
        );

        result.or_else(|| cache.spans(logical_line)).unwrap_or_else(|| {
            Rc::new(vec![(
                self.style.text_color,
                self.buffer.line(logical_line).to_string(),
            )])
        })
    }

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
                        expand_tabs_visible(segment_text, super::TAB_WIDTH)
                    } else {
                        expand_tabs(segment_text, super::TAB_WIDTH).into_owned()
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
                expand_tabs_visible(line_segment, super::TAB_WIDTH)
            } else {
                expand_tabs(line_segment, super::TAB_WIDTH).into_owned()
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

    /// Draws bracket-pair colorization (rainbow brackets) for `visual_line`.
    ///
    /// Each `( ) [ ] { }` character on the line is redrawn on top of the
    /// already-rendered syntax-highlighted text, colored by its nesting
    /// depth (see [`super::bracket_match::bracket_depth_indices`]) so a
    /// matching pair always shares the same color, cycling through
    /// [`BRACKET_PAIR_COLORS`] as depth increases. No-op when the feature is
    /// disabled.
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

        let logical_line = visual_line.logical_line;
        let start_depth = self
            .bracket_depth_cache
            .borrow_mut()
            .depth_at_line_start(&self.buffer, logical_line);

        let line_content = self.buffer.line(logical_line);
        let indices = super::bracket_match::bracket_depth_indices(
            line_content,
            start_depth,
        );

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
                color: BRACKET_PAIR_COLORS[depth % BRACKET_PAIR_COLORS.len()],
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
    use std::rc::Rc;

    use syntect::highlighting::ThemeSet;
    use syntect::parsing::SyntaxSet;

    use super::*;
    use crate::canvas_editor::{CHAR_WIDTH, FONT_SIZE, compare_floats};

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
    fn test_highlight_line_spans_covers_full_line() {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax = syntax_set.find_syntax_plain_text();
        let theme = syntect::highlighting::Theme::default();

        let line = "fn main() {}";
        let spans = highlight_line_spans(line, syntax, &theme, &syntax_set);

        assert!(!spans.is_empty(), "expected at least one span");
        let combined: String =
            spans.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(combined, line, "spans must cover the entire line");
    }

    #[test]
    fn test_highlighted_line_cached_reuses_until_invalidated() {
        let editor = CodeEditor::new("fn main() {}\nlet x = 1;", "rs");
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax = syntax_set.find_syntax_plain_text();
        let theme = syntect::highlighting::Theme::default();

        let first =
            editor.highlighted_line_cached(0, syntax, &theme, &syntax_set);
        let second =
            editor.highlighted_line_cached(0, syntax, &theme, &syntax_set);
        assert!(
            Rc::ptr_eq(&first, &second),
            "a cached line should be reused as the same Rc"
        );

        editor.invalidate_highlight_from(0);
        let third =
            editor.highlighted_line_cached(0, syntax, &theme, &syntax_set);
        assert!(
            !Rc::ptr_eq(&first, &third),
            "invalidation should force the line to be recomputed"
        );
    }

    #[test]
    fn test_highlight_budget_uses_plain_fallback_without_scanning_to_target() {
        let editor = CodeEditor::new("zero\none\ntwo\nthree\nfour", "txt");
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax = syntax_set.find_syntax_plain_text();
        let theme = syntect::highlighting::Theme::default();
        editor.highlight_lines_remaining.set(2);

        let spans =
            editor.highlighted_line_cached(4, syntax, &theme, &syntax_set);
        let combined: String =
            spans.iter().map(|(_, text)| text.as_str()).collect();

        assert_eq!(combined, "four");
        assert_eq!(
            editor
                .highlight_cache
                .borrow()
                .as_ref()
                .map(super::super::HighlightCache::valid_len),
            Some(2)
        );
        assert_eq!(editor.highlight_lines_remaining.get(), 0);
    }

    #[test]
    fn test_highlighted_line_cached_handles_multiline_comments() {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax = syntax_set
            .find_syntax_by_extension("rs")
            .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
        let theme = ThemeSet::load_defaults()
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_default();

        // Line index 2 ("still inside") sits within a `/* ... */` block.
        let code = "let a = 1;\n/* open\nstill inside\n*/\nlet b = 2;";
        let editor = CodeEditor::new(code, "rs");

        // Sequential highlighting resumes inside the block comment.
        let sequential =
            editor.highlighted_line_cached(2, syntax, &theme, &syntax_set);
        // Independent highlighting wrongly treats the line as ordinary code.
        let independent = highlight_line_spans(
            editor.buffer.line(2),
            syntax,
            &theme,
            &syntax_set,
        );

        let sequential_color = sequential.first().map(|(color, _)| *color);
        let independent_color = independent.first().map(|(color, _)| *color);
        assert!(sequential_color.is_some());
        assert!(independent_color.is_some());
        assert_ne!(
            sequential_color, independent_color,
            "a line inside a block comment must be colored as a comment"
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
