//! Layout metrics: font/character/line dimensions, viewport size, and the
//! gutter-width constants and calculations derived from them.

use iced::advanced::text::{
    Alignment, Paragraph, Renderer as TextRenderer, Text,
};
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use unicode_width::UnicodeWidthChar;

use crate::canvas_editor::CodeEditor;

/// Canvas-based text editor constants
pub(crate) const FONT_SIZE: f32 = 14.0;
pub(crate) const LINE_HEIGHT: f32 = 20.0;
pub(crate) const CHAR_WIDTH: f32 = 8.4; // Monospace character width
pub(crate) const TAB_WIDTH: usize = 4;
pub(crate) const GUTTER_WIDTH: f32 = 45.0;
/// Width in pixels of the fold margin (chevron column) added to the gutter when
/// code folding is enabled.
pub(crate) const FOLD_MARGIN_WIDTH: f32 = 14.0;
pub(crate) const CURSOR_BLINK_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(530);

/// Reference glyph used to measure wide (CJK) characters.
const WIDE_REFERENCE_GLYPH: &str = "汉";

/// Largest narrow-width drift, in pixels, under which a cached wide width is
/// still trusted (see [`cached_wide_char_width`]).
const NARROW_WIDTH_TOLERANCE: f32 = 0.01;

/// Wide-glyph widths shared by every editor in the process, keyed by font and
/// size bits, each stored with the narrow width measured alongside it.
type WideWidthCache = HashMap<(iced::Font, u32), (f32, f32)>;

/// Process-wide [`WideWidthCache`].
static WIDE_WIDTHS: OnceLock<Mutex<WideWidthCache>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    /// Wide-glyph measurements taken on this thread, counted by the tests.
    static WIDE_MEASUREMENTS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

/// Returns the wide-glyph width of `font` at `size`, measuring it only when
/// no trustworthy cached value exists.
///
/// Measuring the wide reference glyph usually starts a fallback-font search
/// (most code fonts have no CJK glyphs), which costs far more than measuring
/// an ASCII letter. Hosts that build many editors (one per tab or request
/// body) paid it on every construction and every font change.
///
/// The cache is validated with `narrow_width`, a fresh measurement of the
/// narrow glyph: if it no longer matches the narrow width stored with the
/// entry, the font's glyphs changed since (typically a font loaded after the
/// first measurement, which had fallen back to another face), so the wide
/// glyph is measured again and the entry replaced.
///
/// # Arguments
///
/// * `font` - The font to measure
/// * `size` - The font size in pixels
/// * `narrow_width` - The current width of the narrow reference glyph
/// * `measure` - Measures the wide reference glyph
fn cached_wide_char_width(
    font: iced::Font,
    size: f32,
    narrow_width: f32,
    measure: impl FnOnce() -> f32,
) -> f32 {
    let key = (font, size.to_bits());
    let cache = WIDE_WIDTHS.get_or_init(Mutex::default);
    let cached = cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&key)
        .copied();
    if let Some((narrow, wide)) = cached
        && (narrow - narrow_width).abs() <= NARROW_WIDTH_TOLERANCE
    {
        return wide;
    }

    #[cfg(test)]
    WIDE_MEASUREMENTS.with(|count| count.set(count.get() + 1));
    let wide = measure();
    cache
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(key, (narrow_width, wide));
    wide
}

/// Measures the width of a single character.
///
/// # Arguments
///
/// * `c` - The character to measure
/// * `full_char_width` - The width of a full-width character
/// * `char_width` - The width of the character
///
/// # Returns
///
/// The calculated width of the character as a `f32`
pub(crate) fn measure_char_width(
    c: char,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    if c == '\t' {
        return char_width * TAB_WIDTH as f32;
    }
    match c.width() {
        Some(w) if w > 1 => full_char_width,
        Some(_) => char_width,
        None => 0.0,
    }
}

/// Computes the visual indentation width of a line, expanding tabs.
///
/// Returns `None` for blank lines (empty or whitespace-only), which have no
/// meaningful indentation and act as "transparent" lines: both code folding and
/// indentation guides infer their level from the surrounding non-blank lines
/// instead.
///
/// The width is expressed in display columns, not character indices: a tab
/// counts as [`TAB_WIDTH`] columns, matching what [`measure_char_width`] does
/// when rendering.
///
/// # Arguments
///
/// * `line` - The line content (without the trailing newline)
///
/// # Returns
///
/// The indentation width in display columns, or `None` for a blank line.
pub(crate) fn indent_width(line: &str) -> Option<usize> {
    let mut width = 0;
    for c in line.chars() {
        match c {
            '\t' => width += TAB_WIDTH,
            ' ' => width += 1,
            _ if c.is_whitespace() => width += 1,
            _ => return Some(width),
        }
    }
    // Reached end of line without a non-whitespace character: blank line.
    None
}

/// Measures rendered text width, accounting for CJK wide characters.
///
/// - Wide characters (e.g. Chinese) use FONT_SIZE.
/// - Narrow characters (e.g. Latin) use CHAR_WIDTH.
/// - Control characters (except tab) have width 0.
///
/// # Arguments
///
/// * `text` - The text string to measure
/// * `full_char_width` - The width of a full-width character
/// * `char_width` - The width of a regular character
///
/// # Returns
///
/// The total calculated width of the text as a `f32`, never negative zero
/// (see the empty-line note below).
///
/// # Note
///
/// `Iterator::sum` over an empty `f32` sequence — a blank line — yields
/// `-0.0`, not `0.0` (`[0f32; 0].iter().sum::<f32>().is_sign_negative()` is
/// `true`). Left as-is, that bit pattern corrupts callers that order widths
/// by `f32::to_bits` (as [`MaxContentWidthCache`](super::caches::MaxContentWidthCache)
/// does): `-0.0`'s sign bit makes it sort *above* every positive width as an
/// unsigned integer, so a single blank line — found in virtually every real
/// file — silently became the reported "widest line" (issue #26). Normalizing
/// the result here keeps `measure_text_width` a sound building block for any
/// such bit-based comparison.
pub(crate) fn measure_text_width(
    text: &str,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    let width: f32 = text
        .chars()
        .map(|c| measure_char_width(c, full_char_width, char_width))
        .sum();
    // Every term above is non-negative, so a negative sum is only reachable
    // as `-0.0` (an empty sum). `.abs()` canonicalizes that one case to
    // `0.0` without a lossy or lint-unfriendly float equality comparison.
    if width.is_sign_negative() { width.abs() } else { width }
}

/// Epsilon value for floating-point comparisons in text layout.
pub(crate) const EPSILON: f32 = 0.001;
/// Multiplier used to extend the cached render window beyond the visible range.
/// The cache window margin is computed as:
///     margin = visible_lines_count * CACHE_WINDOW_MARGIN_MULTIPLIER
/// A larger margin reduces how often we clear and rebuild the canvas cache when
/// scrolling, improving performance on very large files while still ensuring
/// correct initial rendering during the first scroll.
pub(crate) const CACHE_WINDOW_MARGIN_MULTIPLIER: usize = 2;
/// Maximum number of previously unseen logical lines syntect may parse while
/// rebuilding one content frame.
///
/// Syntax state is sequential, so jumping far into a large file can otherwise
/// parse every line from the start in one blocking draw call. Lines beyond this
/// budget temporarily use the editor's plain text color and will be highlighted
/// as later content redraws advance the cached parser state.
pub(crate) const HIGHLIGHT_LINES_PER_FRAME: usize = 2_000;

/// Wall-clock budget for sequential syntect catch-up within one content frame.
///
/// A line count alone cannot bound frame time: a line's cost varies a lot
/// with syntax, line length and build profile (a debug build is roughly ten
/// times slower), so [`HIGHLIGHT_LINES_PER_FRAME`] lines can take well over
/// a frame. This caps the time as well; 8 ms keeps a frame inside a 60 fps
/// budget while still making steady progress.
pub(crate) const HIGHLIGHT_TIME_PER_FRAME: std::time::Duration =
    std::time::Duration::from_millis(8);

/// Lines parsed between two clock reads during highlight catch-up, so the
/// time check itself stays negligible.
pub(crate) const HIGHLIGHT_CLOCK_CHECK_INTERVAL: usize = 64;

/// Compares two floating point numbers with a small epsilon tolerance.
///
/// # Arguments
///
/// * `a` - first float number
/// * `b` - second float number
///
/// # Returns
///
/// * `Ordering::Equal` if `abs(a - b) < EPSILON`
/// * `Ordering::Greater` if `a > b` (and not equal)
/// * `Ordering::Less` if `a < b` (and not equal)
pub(crate) fn compare_floats(a: f32, b: f32) -> CmpOrdering {
    if (a - b).abs() < EPSILON {
        CmpOrdering::Equal
    } else if a > b {
        CmpOrdering::Greater
    } else {
        CmpOrdering::Less
    }
}

impl CodeEditor {
    /// Sets the font used by the editor
    ///
    /// Character dimensions are re-measured with the new font, but the line
    /// height is left alone — use [`Self::set_line_height`] to change it.
    ///
    /// # Arguments
    ///
    /// * `font` - The iced font to set for the editor
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_font(iced::Font::MONOSPACE);
    ///
    /// // Character widths are re-measured for the new font.
    /// assert!(editor.char_width() > 0.0);
    /// ```
    pub fn set_font(&mut self, font: iced::Font) {
        self.font = font;
        self.recalculate_char_dimensions(false);
    }

    /// Sets the font size and recalculates character dimensions.
    ///
    /// If `auto_adjust_line_height` is true, `line_height` will also be scaled to maintain
    /// the default proportion (Line Height ~ 1.43x).
    ///
    /// # Arguments
    ///
    /// * `size` - The font size in pixels
    /// * `auto_adjust_line_height` - Whether to automatically adjust the line height
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // Scale the line height along with the font, the usual "zoom" case.
    /// editor.set_font_size(28.0, true);
    /// assert!((editor.font_size() - 28.0).abs() < f32::EPSILON);
    /// // Doubling 14px doubles the 20px default line height.
    /// assert!((editor.line_height() - 40.0).abs() < f32::EPSILON);
    ///
    /// // Or keep a line height the host controls itself.
    /// editor.set_font_size(14.0, false);
    /// assert!((editor.line_height() - 40.0).abs() < f32::EPSILON);
    /// ```
    pub fn set_font_size(&mut self, size: f32, auto_adjust_line_height: bool) {
        self.font_size = size;
        self.recalculate_char_dimensions(auto_adjust_line_height);
    }

    /// Recalculates character dimensions based on current font and size.
    pub(crate) fn recalculate_char_dimensions(
        &mut self,
        auto_adjust_line_height: bool,
    ) {
        self.char_width = self.measure_single_char_width("a");
        // Use '汉' as a standard reference for CJK (Chinese, Japanese, Korean)
        // wide characters. It is the costly one to measure, so it is shared
        // across editors and validated by the fresh narrow width.
        self.full_char_width = cached_wide_char_width(
            self.font,
            self.font_size,
            self.char_width,
            || self.measure_single_char_width(WIDE_REFERENCE_GLYPH),
        );

        // Fallback for infinite width measurements
        if self.char_width.is_infinite() {
            self.char_width = self.font_size / 2.0; // Rough estimate for monospace
        }

        if self.full_char_width.is_infinite() {
            self.full_char_width = self.font_size;
        }

        if auto_adjust_line_height {
            let line_height_ratio = LINE_HEIGHT / FONT_SIZE;
            self.line_height = self.font_size * line_height_ratio;
        }

        self.content_cache.clear();
        self.overlay_cache.clear();
        *self.max_content_width_cache.borrow_mut() = None;
    }

    /// Measures the width of a single character string using the current font settings.
    fn measure_single_char_width(&self, content: &str) -> f32 {
        let text = Text {
            content,
            font: self.font,
            size: iced::Pixels(self.font_size),
            line_height: iced::advanced::text::LineHeight::default(),
            bounds: iced::Size::new(f32::INFINITY, f32::INFINITY),
            align_x: Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: iced::advanced::text::Shaping::Advanced,
            wrapping: iced::advanced::text::Wrapping::default(),
        };
        let p = <iced::Renderer as TextRenderer>::Paragraph::with_text(text);
        p.min_width()
    }

    /// Returns the current font size.
    ///
    /// # Returns
    ///
    /// The font size in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!((editor.font_size() - 14.0).abs() < f32::EPSILON);
    /// ```
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the width of a standard narrow character in pixels.
    ///
    /// Measured from the current font rather than assumed, so it stays correct
    /// after [`Self::set_font`] or [`Self::set_font_size`].
    ///
    /// # Returns
    ///
    /// The character width in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// let narrow = editor.char_width();
    /// assert!(narrow > 0.0);
    ///
    /// // A larger font measures wider characters.
    /// editor.set_font_size(28.0, true);
    /// assert!(editor.char_width() > narrow);
    /// ```
    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    /// Returns the width of a wide character (e.g. CJK) in pixels.
    ///
    /// Measured from `汉` as the reference glyph. Kept separate from
    /// [`Self::char_width`] so mixed-script lines are measured correctly
    /// instead of assuming every character is one cell wide.
    ///
    /// # Returns
    ///
    /// The full character width in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // A wide glyph is never narrower than a narrow one.
    /// assert!(editor.full_char_width() >= editor.char_width());
    /// ```
    pub fn full_char_width(&self) -> f32 {
        self.full_char_width
    }

    /// Measures the rendered width for a given text snippet using editor metrics.
    ///
    /// Each character counts as either [`Self::char_width`] or
    /// [`Self::full_char_width`] depending on whether it is a wide glyph, so
    /// the result is correct for mixed-script text.
    ///
    /// # Arguments
    ///
    /// * `text` - The snippet to measure
    ///
    /// # Returns
    ///
    /// The width in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!((editor.measure_text_width("") - 0.0).abs() < f32::EPSILON);
    ///
    /// // Width grows with the text.
    /// assert!(editor.measure_text_width("hello") > editor.measure_text_width("hi"));
    /// ```
    pub fn measure_text_width(&self, text: &str) -> f32 {
        measure_text_width(text, self.full_char_width, self.char_width)
    }

    /// Sets the line height used by the editor
    ///
    /// # Arguments
    ///
    /// * `height` - The line height in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_line_height(24.0);
    /// assert!((editor.line_height() - 24.0).abs() < f32::EPSILON);
    /// ```
    pub fn set_line_height(&mut self, height: f32) {
        self.line_height = height;
        self.content_cache.clear();
        self.overlay_cache.clear();
    }

    /// Returns the current line height.
    ///
    /// # Returns
    ///
    /// The line height in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!((editor.line_height() - 20.0).abs() < f32::EPSILON);
    /// ```
    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    /// Returns the current viewport height in pixels.
    ///
    /// Set by the layout each frame; the default applies only until the
    /// editor has been drawn once.
    ///
    /// # Returns
    ///
    /// The viewport height in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_viewport_height(480.0);
    /// assert!((editor.viewport_height() - 480.0).abs() < f32::EPSILON);
    /// ```
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Returns the current viewport width in pixels.
    ///
    /// This is the width line wrapping is computed against when no fixed wrap
    /// column is configured.
    ///
    /// # Returns
    ///
    /// The viewport width in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!(editor.viewport_width() > 0.0);
    /// ```
    pub fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    /// Returns the current vertical scroll offset in pixels.
    ///
    /// Zero means the first line is at the top of the viewport.
    ///
    /// # Returns
    ///
    /// The scroll offset in pixels
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // A fresh editor is scrolled to the top.
    /// assert!((editor.viewport_scroll() - 0.0).abs() < f32::EPSILON);
    /// ```
    pub fn viewport_scroll(&self) -> f32 {
        self.viewport_scroll
    }

    /// Sets the viewport height for the editor.
    ///
    /// This determines the minimum height of the canvas, ensuring proper
    /// background rendering even when content is smaller than the viewport.
    ///
    /// # Arguments
    ///
    /// * `height` - The viewport height in pixels
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_viewport_height(500.0);
    /// ```
    #[must_use]
    pub fn with_viewport_height(mut self, height: f32) -> Self {
        self.viewport_height = height;
        self
    }

    /// Returns the total gutter width, including the line-number area and the
    /// fold margin.
    ///
    /// The fold margin is added when folding is enabled, independently of line
    /// numbers, so fold chevrons remain clickable even without line numbers.
    pub(crate) fn gutter_width(&self) -> f32 {
        self.line_number_gutter_width() + self.fold_margin_width()
    }

    /// Returns the width of the line-number area (excluding the fold margin).
    pub(crate) fn line_number_gutter_width(&self) -> f32 {
        if self.line_numbers_enabled { GUTTER_WIDTH } else { 0.0 }
    }

    /// Returns the width of the fold margin (the chevron column), or `0.0` when
    /// folding is disabled or fold markers are never shown.
    pub(crate) fn fold_margin_width(&self) -> f32 {
        if self.folding_enabled
            && self.fold_markers != crate::FoldMarkers::Never
        {
            FOLD_MARGIN_WIDTH
        } else {
            0.0
        }
    }

    /// Returns the horizontal extents of the gutter columns, in canvas
    /// coordinates.
    ///
    /// The single source of truth for the gutter layout: drawing, fold
    /// clicks, the gutter background and sticky headers all read it, so the
    /// fold-marker column can move to either side of the line numbers.
    pub(crate) fn gutter_columns(&self) -> GutterColumns {
        let numbers = self.line_number_gutter_width();
        let markers = self.fold_margin_width();
        match self.fold_marker_position {
            crate::FoldMarkerPosition::AfterLineNumbers => GutterColumns {
                line_numbers: 0.0..numbers,
                fold_markers: numbers..numbers + markers,
            },
            crate::FoldMarkerPosition::BeforeLineNumbers => GutterColumns {
                fold_markers: 0.0..markers,
                line_numbers: markers..markers + numbers,
            },
        }
    }
}

/// Horizontal extents of the gutter's two columns, in canvas coordinates.
///
/// Either range is empty when its column is hidden (line numbers disabled,
/// folding disabled or fold markers never shown).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GutterColumns {
    /// The line-number column.
    pub(crate) line_numbers: std::ops::Range<f32>,
    /// The fold-marker (chevron) column.
    pub(crate) fold_markers: std::ops::Range<f32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indent_width_blank_lines() {
        assert_eq!(indent_width(""), None);
        assert_eq!(indent_width("   "), None);
        assert_eq!(indent_width("\t"), None);
    }

    #[test]
    fn test_indent_width_expands_tabs() {
        assert_eq!(indent_width("code"), Some(0));
        assert_eq!(indent_width("  code"), Some(2));
        assert_eq!(indent_width("\tcode"), Some(TAB_WIDTH));
        assert_eq!(indent_width("\t  code"), Some(TAB_WIDTH + 2));
    }

    #[test]
    fn test_compare_floats() {
        // Equal cases
        assert_eq!(
            compare_floats(1.0, 1.0),
            CmpOrdering::Equal,
            "Exact equality"
        );
        assert_eq!(
            compare_floats(1.0, 1.0 + 0.0001),
            CmpOrdering::Equal,
            "Within epsilon (positive)"
        );
        assert_eq!(
            compare_floats(1.0, 1.0 - 0.0001),
            CmpOrdering::Equal,
            "Within epsilon (negative)"
        );

        // Greater cases
        assert_eq!(
            compare_floats(1.0 + 0.002, 1.0),
            CmpOrdering::Greater,
            "Definitely greater"
        );
        assert_eq!(
            compare_floats(1.0011, 1.0),
            CmpOrdering::Greater,
            "Just above epsilon"
        );

        // Less cases
        assert_eq!(
            compare_floats(1.0, 1.0 + 0.002),
            CmpOrdering::Less,
            "Definitely less"
        );
        assert_eq!(
            compare_floats(1.0, 1.0011),
            CmpOrdering::Less,
            "Just below negative epsilon"
        );
    }

    #[test]
    fn test_measure_text_width_ascii() {
        // "abc" (3 chars) -> 3 * CHAR_WIDTH
        let text = "abc";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = CHAR_WIDTH * 3.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for ASCII"
        );
    }

    #[test]
    fn test_measure_text_width_cjk() {
        // "你好" (2 chars) -> 2 * FONT_SIZE
        // Chinese characters are typically full-width.
        // width = 2 * FONT_SIZE
        let text = "你好";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = FONT_SIZE * 2.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for CJK"
        );
    }

    #[test]
    fn test_measure_text_width_mixed() {
        // "Hi" (2 chars) -> 2 * CHAR_WIDTH
        // "你好" (2 chars) -> 2 * FONT_SIZE
        let text = "Hi你好";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = CHAR_WIDTH * 2.0 + FONT_SIZE * 2.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for mixed content"
        );
    }

    #[test]
    fn test_measure_text_width_control_chars() {
        // "\t\n" (2 chars)
        // width = 4 * CHAR_WIDTH (tab) + 0 (newline)
        let text = "\t\n";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = CHAR_WIDTH * TAB_WIDTH as f32;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for control chars"
        );
    }

    #[test]
    fn test_measure_text_width_empty() {
        let text = "";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        assert!(
            (width - 0.0).abs() < f32::EPSILON,
            "Width should be 0 for empty string"
        );
        // Regression test for issue #26: `Iterator::sum` over zero `f32`
        // terms yields `-0.0`, not `0.0`. Left uncorrected, that sign bit
        // makes an empty line's width sort *above* every real line's width
        // in code that orders by `f32::to_bits` (as `MaxContentWidthCache`
        // does), so a single blank line silently won the "widest line"
        // comparison and hid the horizontal scrollbar entirely.
        assert!(
            !width.is_sign_negative(),
            "an empty line's width must be positive zero, not -0.0"
        );
    }

    #[test]
    fn test_measure_text_width_emoji() {
        // "👋" (1 char, width > 1) -> FONT_SIZE
        let text = "👋";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = FONT_SIZE;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for emoji"
        );
    }

    #[test]
    fn test_measure_text_width_korean() {
        // "안녕하세요" (5 chars)
        // Korean characters are typically full-width.
        // width = 5 * FONT_SIZE
        let text = "안녕하세요";
        let width = measure_text_width(text, FONT_SIZE, CHAR_WIDTH);
        let expected = FONT_SIZE * 5.0;
        assert_eq!(
            compare_floats(width, expected),
            CmpOrdering::Equal,
            "Width mismatch for Korean"
        );
    }

    #[test]
    fn test_measure_text_width_japanese() {
        // "こんにちは" (Hiragana, 5 chars) -> 5 * FONT_SIZE
        // "カタカナ" (Katakana, 4 chars) -> 4 * FONT_SIZE
        // "漢字" (Kanji, 2 chars) -> 2 * FONT_SIZE

        let text_hiragana = "こんにちは";
        let width_hiragana =
            measure_text_width(text_hiragana, FONT_SIZE, CHAR_WIDTH);
        let expected_hiragana = FONT_SIZE * 5.0;
        assert_eq!(
            compare_floats(width_hiragana, expected_hiragana),
            CmpOrdering::Equal,
            "Width mismatch for Hiragana"
        );

        let text_katakana = "カタカナ";
        let width_katakana =
            measure_text_width(text_katakana, FONT_SIZE, CHAR_WIDTH);
        let expected_katakana = FONT_SIZE * 4.0;
        assert_eq!(
            compare_floats(width_katakana, expected_katakana),
            CmpOrdering::Equal,
            "Width mismatch for Katakana"
        );

        let text_kanji = "漢字";
        let width_kanji = measure_text_width(text_kanji, FONT_SIZE, CHAR_WIDTH);
        let expected_kanji = FONT_SIZE * 2.0;
        assert_eq!(
            compare_floats(width_kanji, expected_kanji),
            CmpOrdering::Equal,
            "Width mismatch for Kanji"
        );
    }

    #[test]
    fn test_set_font_size() {
        let mut editor = CodeEditor::new("", "rs");

        // Initial state (defaults)
        assert!((editor.font_size() - 14.0).abs() < f32::EPSILON);
        assert!((editor.line_height() - 20.0).abs() < f32::EPSILON);

        // Test auto adjust = true
        editor.set_font_size(28.0, true);
        assert!((editor.font_size() - 28.0).abs() < f32::EPSILON);
        // Line height should double: 20.0 * (28.0/14.0) = 40.0
        assert_eq!(
            compare_floats(editor.line_height(), 40.0),
            CmpOrdering::Equal
        );

        // Test auto adjust = false
        // First set line height to something custom
        editor.set_line_height(50.0);
        // Change font size but keep line height
        editor.set_font_size(14.0, false);
        assert!((editor.font_size() - 14.0).abs() < f32::EPSILON);
        // Line height should stay 50.0
        assert_eq!(
            compare_floats(editor.line_height(), 50.0),
            CmpOrdering::Equal
        );
        // Char width should have scaled back to roughly default (but depends on measurement)
        // We check if it is close to the expected value, but since measurement can vary,
        // we just ensure it is positive and close to what we expect (around 8.4)
        assert!(editor.char_width > 0.0);
        assert!((editor.char_width - CHAR_WIDTH).abs() < 0.5);
    }

    #[test]
    fn test_measure_single_char_width() {
        let editor = CodeEditor::new("", "rs");

        // Measure 'a'
        let width_a = editor.measure_single_char_width("a");
        assert!(width_a > 0.0, "Width of 'a' should be positive");

        // Measure Chinese char
        let width_cjk = editor.measure_single_char_width("汉");
        assert!(width_cjk > 0.0, "Width of '汉' should be positive");

        assert!(
            width_cjk > width_a,
            "Width of '汉' should be greater than 'a'"
        );

        // Check that width_cjk is roughly double of width_a (common in terminal fonts)
        // but we just check it is significantly larger
        assert!(width_cjk >= width_a * 1.5);
    }

    #[test]
    fn test_gutter_columns_default_to_numbers_then_markers() {
        let editor = CodeEditor::new("fn a() {\n    b\n}", "rs");
        let columns = editor.gutter_columns();

        assert_eq!(columns.line_numbers, 0.0..GUTTER_WIDTH);
        assert_eq!(
            columns.fold_markers,
            GUTTER_WIDTH..GUTTER_WIDTH + FOLD_MARGIN_WIDTH
        );
        assert_eq!(
            columns.fold_markers.end.to_bits(),
            editor.gutter_width().to_bits()
        );
    }

    #[test]
    fn test_marker_position_only_swaps_the_gutter_columns() {
        let mut editor = CodeEditor::new("fn a() {\n    b\n}", "rs");
        let width = editor.gutter_width();
        editor.set_fold_marker_position(
            crate::FoldMarkerPosition::BeforeLineNumbers,
        );
        let columns = editor.gutter_columns();

        assert_eq!(columns.fold_markers, 0.0..FOLD_MARGIN_WIDTH);
        assert_eq!(
            columns.line_numbers,
            FOLD_MARGIN_WIDTH..FOLD_MARGIN_WIDTH + GUTTER_WIDTH
        );
        assert_eq!(
            editor.gutter_width().to_bits(),
            width.to_bits(),
            "the text must not move"
        );
    }

    #[test]
    fn test_never_markers_free_their_column_but_keep_fold_state() {
        let mut editor = CodeEditor::new("fn a() {\n    b\n}", "rs");
        let _ = editor.update(&crate::Message::ToggleFold(0));
        assert!(editor.is_folded(0));

        editor.set_fold_markers(crate::FoldMarkers::Never);

        assert!(editor.gutter_columns().fold_markers.is_empty());
        assert_eq!(editor.gutter_width().to_bits(), GUTTER_WIDTH.to_bits());
        assert!(editor.is_folded(0), "the fold state survives");
        assert_eq!(
            editor.visual_lines_cached(800.0).len(),
            2,
            "the folded block stays hidden"
        );
    }

    #[test]
    fn test_hover_markers_show_on_hover_and_on_collapsed_lines() {
        let mut editor =
            CodeEditor::new("fn a() {\n    b\n}\nfn c() {\n    d\n}", "rs");
        editor.set_fold_markers(crate::FoldMarkers::OnGutterHover);
        let marked = |editor: &CodeEditor| -> Vec<usize> {
            editor
                .visual_lines_cached(800.0)
                .iter()
                .filter(|line| editor.shows_fold_marker(line))
                .map(|line| line.logical_line)
                .collect()
        };

        assert!(marked(&editor).is_empty(), "hidden while not hovered");
        let _ = editor.update(&crate::Message::ToggleFold(3));
        assert_eq!(marked(&editor), vec![3], "a collapsed block stays marked");
        editor.gutter_hovered.set(true);
        assert_eq!(marked(&editor), vec![0, 3], "all shown on hover");
    }

    /// Wide measurements taken so far on this thread.
    fn wide_measurements() -> usize {
        WIDE_MEASUREMENTS.with(std::cell::Cell::get)
    }

    #[test]
    fn test_wide_width_is_measured_once_per_font_and_size() {
        // A font no other test uses, so the process-wide cache starts cold.
        let font = iced::Font::with_name("Wide Width Count Test");
        let mut first = CodeEditor::new("", "rs");
        first.set_font(font);
        let start = wide_measurements();

        first.set_font_size(13.25, true);
        assert_eq!(wide_measurements(), start + 1, "cold cache measures");

        for _ in 0..5 {
            let mut editor = CodeEditor::new("", "rs");
            editor.set_font(font);
            editor.set_font_size(13.25, true);
            assert_eq!(
                editor.full_char_width.to_bits(),
                first.full_char_width.to_bits()
            );
        }
        assert_eq!(wide_measurements(), start + 1, "same font/size: cache hit");

        first.set_font_size(16.75, true);
        assert_eq!(wide_measurements(), start + 2, "a new size measures once");
    }

    #[test]
    fn test_cached_wide_width_matches_a_direct_measurement() {
        let mut editor = CodeEditor::new("", "rs");
        editor.set_font(iced::Font::with_name("Wide Width Match Test"));
        editor.set_font_size(15.5, false);

        let direct = editor.measure_single_char_width(WIDE_REFERENCE_GLYPH);
        assert_eq!(editor.full_char_width.to_bits(), direct.to_bits());
    }

    #[test]
    fn test_stale_wide_width_is_measured_again() {
        let font = iced::Font::with_name("Wide Width Stale Test");
        // Seed an entry whose narrow width no longer matches, as when the
        // font was still falling back to another face at first measurement.
        let _ = cached_wide_char_width(font, 12.0, 999.0, || 1.0);

        let mut editor = CodeEditor::new("", "rs");
        editor.set_font(font);
        let start = wide_measurements();
        editor.set_font_size(12.0, false);

        assert_eq!(wide_measurements(), start + 1, "stale entry re-measured");
        let direct = editor.measure_single_char_width(WIDE_REFERENCE_GLYPH);
        assert_eq!(editor.full_char_width.to_bits(), direct.to_bits());
    }

    #[test]
    fn test_set_line_height() {
        let mut editor = CodeEditor::new("", "rs");

        // Initial state
        assert!((editor.line_height() - LINE_HEIGHT).abs() < f32::EPSILON);

        // Set custom line height
        editor.set_line_height(35.0);
        assert!((editor.line_height() - 35.0).abs() < f32::EPSILON);

        // Font size should remain unchanged
        assert!((editor.font_size() - FONT_SIZE).abs() < f32::EPSILON);
    }
}
