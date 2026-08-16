//! Layout metrics: font/character/line dimensions, viewport size, and the
//! gutter-width constants and calculations derived from them.

use iced::advanced::text::{
    Alignment, Paragraph, Renderer as TextRenderer, Text,
};
use std::cmp::Ordering as CmpOrdering;
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
/// The total calculated width of the text as a `f32`
pub(crate) fn measure_text_width(
    text: &str,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    text.chars()
        .map(|c| measure_char_width(c, full_char_width, char_width))
        .sum()
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
    /// # Arguments
    ///
    /// * `font` - The iced font to set for the editor
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
        // Use '汉' as a standard reference for CJK (Chinese, Japanese, Korean) wide characters
        self.full_char_width = self.measure_single_char_width("汉");

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
    pub fn font_size(&self) -> f32 {
        self.font_size
    }

    /// Returns the width of a standard narrow character in pixels.
    ///
    /// # Returns
    ///
    /// The character width in pixels
    pub fn char_width(&self) -> f32 {
        self.char_width
    }

    /// Returns the width of a wide character (e.g. CJK) in pixels.
    ///
    /// # Returns
    ///
    /// The full character width in pixels
    pub fn full_char_width(&self) -> f32 {
        self.full_char_width
    }

    /// Measures the rendered width for a given text snippet using editor metrics.
    pub fn measure_text_width(&self, text: &str) -> f32 {
        measure_text_width(text, self.full_char_width, self.char_width)
    }

    /// Sets the line height used by the editor
    ///
    /// # Arguments
    ///
    /// * `height` - The line height in pixels
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
    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    /// Returns the current viewport height in pixels.
    pub fn viewport_height(&self) -> f32 {
        self.viewport_height
    }

    /// Returns the current viewport width in pixels.
    pub fn viewport_width(&self) -> f32 {
        self.viewport_width
    }

    /// Returns the current vertical scroll offset in pixels.
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
    /// folding is disabled.
    pub(crate) fn fold_margin_width(&self) -> f32 {
        if self.folding_enabled { FOLD_MARGIN_WIDTH } else { 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
