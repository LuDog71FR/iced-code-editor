//! Detection of color literals for the inline color-preview swatches.
//!
//! An inline color preview is the small colored square an editor draws next to
//! a color written in the source — `#1e1e2e`, `0xFF6B6B`, `rgb(58, 123, 213)` —
//! so the color can be read at a glance instead of decoded mentally. This
//! module holds the pure detection logic, so it can be unit-tested without a
//! renderer; the drawing itself lives in [`crate::canvas_editor::render`].
//!
//! Detection is purely lexical: no syntax awareness is involved, so a literal
//! inside a comment or a string is reported just like one in real code, which
//! is what a reader looking for colors expects.

use iced::Color;

use crate::buffer::TextBuffer;

/// Maximum number of characters scanned while looking for the closing
/// parenthesis of a functional notation such as `rgb(…)`.
///
/// Without a bound, every stray `rgb(` in a long minified line would trigger a
/// scan to the end of that line. No valid color function comes close to this
/// many characters, so the limit only ever rejects malformed input.
const MAX_FUNCTION_SCAN: usize = 64;

/// Number of color components in the functional notations handled here.
const RGB_COMPONENTS: usize = 3;

/// Number of color components once an alpha channel is present.
const RGBA_COMPONENTS: usize = 4;

/// A color literal found in a line of text.
///
/// Columns are character indices into the logical line, matching the
/// convention used by the renderer and by [`crate::buffer::TextBuffer`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorLiteral {
    /// Column of the literal's first character.
    pub start_col: usize,
    /// Column one past the literal's last character.
    pub end_col: usize,
    /// The color the literal denotes.
    pub color: Color,
}

/// Finds every color literal in `line`.
///
/// The recognized notations are:
///
/// * `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` — CSS hexadecimal, with the
///   short forms expanded by digit doubling (`#f0c` is `#ff00cc`)
/// * `0xrrggbb`, `0xrrggbbaa` — the hexadecimal spelling common in Rust
/// * `rgb(…)` and `rgba(…)` — three or four comma-separated components, each
///   an integer, a float or a percentage; the alpha component is a `0.0`–`1.0`
///   ratio or a percentage
///
/// A literal is only recognized when it does not continue an identifier, so
/// `x0xff0000` and `#deadbeefcafe` yield nothing. Hexadecimal runs of any
/// other length are rejected as well: `#12345` is not a color.
///
/// # Arguments
///
/// * `line` - The logical line to scan
///
/// # Returns
///
/// The literals found, ordered by increasing column. Literals never overlap:
/// scanning resumes after the end of each match.
pub(crate) fn color_literals(line: &str) -> Vec<ColorLiteral> {
    let chars: Vec<char> = line.chars().collect();
    let mut literals = Vec::new();
    let mut col = 0;

    while col < chars.len() {
        if starts_new_token(&chars, col)
            && let Some(literal) = literal_at(&chars, col)
        {
            col = literal.end_col;
            literals.push(literal);
            continue;
        }
        col += 1;
    }

    literals
}

/// Reuses one logical line's literals across the visual segments it wraps
/// into.
///
/// [`color_literals`] allocates and scans the whole logical line, but the
/// renderer walks *visual* lines: without this, a line soft-wrapped into sixty
/// segments would be scanned sixty times for the same result, since a literal
/// can sit in any one of them.
///
/// The memo is keyed on the line index alone, so it is only valid for as long
/// as the buffer cannot change — that is, within a single draw pass. Build one
/// per pass and drop it; never keep one across edits.
#[derive(Debug, Default)]
pub(crate) struct LineLiterals {
    /// Line the memo currently holds, if any.
    line: Option<usize>,
    /// Literals found on that line.
    literals: Vec<ColorLiteral>,
}

impl LineLiterals {
    /// Returns the literals on `line`, scanning it only when it differs from
    /// the line asked for last.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The buffer the line belongs to
    /// * `line` - Index of the logical line to scan
    ///
    /// # Returns
    ///
    /// The literals found, ordered by increasing column
    pub(crate) fn get(
        &mut self,
        buffer: &TextBuffer,
        line: usize,
    ) -> &[ColorLiteral] {
        if self.line != Some(line) {
            self.literals = color_literals(buffer.line(line));
            self.line = Some(line);
        }

        &self.literals
    }
}

/// Returns whether the character at `col` can begin a color literal.
///
/// A literal never continues a word, so the preceding character must not be
/// alphanumeric or an underscore. This is what keeps the `0xff0000` inside
/// `raw0xff0000` from being read as a color.
///
/// # Arguments
///
/// * `chars` - The line, as characters
/// * `col` - Column of the candidate first character
fn starts_new_token(chars: &[char], col: usize) -> bool {
    match col.checked_sub(1).and_then(|previous| chars.get(previous)) {
        Some(previous) => !previous.is_alphanumeric() && *previous != '_',
        None => true,
    }
}

/// Parses the color literal starting at `col`, if there is one.
///
/// # Arguments
///
/// * `chars` - The line, as characters
/// * `col` - Column of the candidate first character
///
/// # Returns
///
/// The literal, or `None` when no notation matches at this position.
fn literal_at(chars: &[char], col: usize) -> Option<ColorLiteral> {
    hexadecimal_at(chars, col).or_else(|| functional_at(chars, col))
}

/// Parses a `#`- or `0x`-prefixed hexadecimal literal starting at `col`.
///
/// The digit run is taken at its maximum length and then validated, so a run
/// of five or seven digits is rejected outright rather than being truncated to
/// a shorter valid form.
///
/// # Arguments
///
/// * `chars` - The line, as characters
/// * `col` - Column of the `#` or of the `0` of `0x`
///
/// # Returns
///
/// The literal, or `None` when the prefix, the digit count or the character
/// following the digits does not fit.
fn hexadecimal_at(chars: &[char], col: usize) -> Option<ColorLiteral> {
    let prefix_len = match chars.get(col)? {
        '#' => 1,
        '0' if matches!(chars.get(col + 1), Some('x' | 'X')) => 2,
        _ => return None,
    };

    let digits_start = col + prefix_len;
    let digits: String = chars[digits_start.min(chars.len())..]
        .iter()
        .take_while(|character| character.is_ascii_hexdigit())
        .collect();
    let end_col = digits_start + digits.chars().count();

    // A trailing letter means the run is part of a longer word, not a color.
    if chars
        .get(end_col)
        .is_some_and(|next| next.is_alphanumeric() || *next == '_')
    {
        return None;
    }

    let color = match (prefix_len, digits.len()) {
        (1, 3 | 4) => expand_short_hexadecimal(&digits),
        (1, 6 | 8) | (2, 6 | 8) => parse_hexadecimal(&digits),
        _ => None,
    }?;

    Some(ColorLiteral { start_col: col, end_col, color })
}

/// Expands a three- or four-digit hexadecimal color by doubling each digit.
///
/// # Arguments
///
/// * `digits` - The digit run, without its `#` prefix
///
/// # Returns
///
/// The color, or `None` when a digit is not hexadecimal.
fn expand_short_hexadecimal(digits: &str) -> Option<Color> {
    let expanded: String =
        digits.chars().flat_map(|digit| [digit, digit]).collect();

    parse_hexadecimal(&expanded)
}

/// Parses a six- or eight-digit hexadecimal color.
///
/// # Arguments
///
/// * `digits` - The digit run, without its `#` or `0x` prefix
///
/// # Returns
///
/// The color, or `None` when the length is not six or eight digits or when a
/// pair does not parse.
fn parse_hexadecimal(digits: &str) -> Option<Color> {
    let byte_at = |index: usize| -> Option<u8> {
        let pair = digits.get(index * 2..index * 2 + 2)?;
        u8::from_str_radix(pair, 16).ok()
    };

    let (red, green, blue) = (byte_at(0)?, byte_at(1)?, byte_at(2)?);
    let alpha = match digits.len() {
        6 => 1.0,
        8 => f32::from(byte_at(3)?) / 255.0,
        _ => return None,
    };

    Some(Color::from_rgba8(red, green, blue, alpha))
}

/// Parses an `rgb(…)` or `rgba(…)` literal starting at `col`.
///
/// The function name is matched case-insensitively, and either name accepts
/// three or four components — as CSS does since Color Level 4.
///
/// # Arguments
///
/// * `chars` - The line, as characters
/// * `col` - Column of the `r` of `rgb`/`rgba`
///
/// # Returns
///
/// The literal, or `None` when the name, the parentheses or a component does
/// not fit.
fn functional_at(chars: &[char], col: usize) -> Option<ColorLiteral> {
    let name_len = ["rgba", "rgb"].into_iter().find_map(|name| {
        let end = col + name.len();
        let matches_name = chars.len() >= end
            && chars[col..end].iter().zip(name.chars()).all(
                |(candidate, expected)| {
                    candidate.to_ascii_lowercase() == expected
                },
            )
            && matches!(chars.get(end), Some('('));

        matches_name.then_some(name.len())
    })?;

    let arguments_start = col + name_len + 1;
    let close_offset = chars
        .get(arguments_start..)?
        .iter()
        .take(MAX_FUNCTION_SCAN)
        .position(|character| *character == ')')?;
    let arguments: String =
        chars[arguments_start..arguments_start + close_offset].iter().collect();

    Some(ColorLiteral {
        start_col: col,
        end_col: arguments_start + close_offset + 1,
        color: parse_components(&arguments)?,
    })
}

/// Parses the comma-separated component list of a functional notation.
///
/// # Arguments
///
/// * `arguments` - The text between the parentheses
///
/// # Returns
///
/// The color, or `None` when the component count is not three or four, or when
/// a component does not parse.
fn parse_components(arguments: &str) -> Option<Color> {
    let components: Vec<&str> = arguments.split(',').collect();
    if !matches!(components.len(), RGB_COMPONENTS | RGBA_COMPONENTS) {
        return None;
    }

    let mut channels = [0_u8; RGB_COMPONENTS];
    for (channel, component) in
        channels.iter_mut().zip(components.iter().copied())
    {
        *channel = parse_channel(component)?;
    }

    let alpha = match components.get(RGBA_COMPONENTS - 1).copied() {
        Some(component) => parse_alpha(component)?,
        None => 1.0,
    };

    Some(Color::from_rgba8(channels[0], channels[1], channels[2], alpha))
}

/// Parses one red, green or blue component.
///
/// Accepts an integer or float in `0`–`255`, or a percentage of `255`.
///
/// # Arguments
///
/// * `component` - The component text, possibly surrounded by whitespace
///
/// # Returns
///
/// The channel value, or `None` when the text does not parse as a number.
fn parse_channel(component: &str) -> Option<u8> {
    let ratio = parse_ratio(component, 255.0)?;

    Some(ratio.clamp(0.0, 255.0).round() as u8)
}

/// Parses the alpha component of a functional notation.
///
/// Accepts a `0.0`–`1.0` ratio or a percentage.
///
/// # Arguments
///
/// * `component` - The component text, possibly surrounded by whitespace
///
/// # Returns
///
/// The opacity, or `None` when the text does not parse as a number.
fn parse_alpha(component: &str) -> Option<f32> {
    let ratio = parse_ratio(component, 1.0)?;

    Some(ratio.clamp(0.0, 1.0))
}

/// Parses a number that may be written as a percentage.
///
/// # Arguments
///
/// * `component` - The component text, possibly surrounded by whitespace
/// * `full_scale` - Value a percentage of `100%` stands for
///
/// # Returns
///
/// The parsed value, or `None` when the text does not parse as a number.
fn parse_ratio(component: &str, full_scale: f32) -> Option<f32> {
    let component = component.trim();

    match component.strip_suffix('%') {
        Some(percentage) => Some(
            percentage.trim_end().parse::<f32>().ok()? / 100.0 * full_scale,
        ),
        None => component.parse::<f32>().ok(),
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::{ColorLiteral, LineLiterals, TextBuffer, color_literals};
    use crate::canvas_editor::compare_floats;
    use iced::Color;

    /// Asserts that `line` holds exactly one literal, spanning `start_col`
    /// to `end_col` and denoting `expected`.
    fn assert_single(
        line: &str,
        start_col: usize,
        end_col: usize,
        expected: Color,
    ) {
        let literals = color_literals(line);
        assert_eq!(literals.len(), 1, "expected one literal in {line:?}");

        let ColorLiteral { start_col: found_start, end_col: found_end, color } =
            literals[0];
        assert_eq!(found_start, start_col, "start column in {line:?}");
        assert_eq!(found_end, end_col, "end column in {line:?}");

        for (found, expected) in [
            (color.r, expected.r),
            (color.g, expected.g),
            (color.b, expected.b),
            (color.a, expected.a),
        ] {
            assert_eq!(
                compare_floats(found, expected),
                Ordering::Equal,
                "channel mismatch in {line:?}"
            );
        }
    }

    #[test]
    fn test_six_digit_hexadecimal() {
        assert_single(
            "background: #1e1e2e;",
            12,
            19,
            Color::from_rgb8(0x1E, 0x1E, 0x2E),
        );
    }

    #[test]
    fn test_three_digit_hexadecimal_is_expanded() {
        assert_single("color: #f0c", 7, 11, Color::from_rgb8(0xFF, 0x00, 0xCC));
    }

    #[test]
    fn test_eight_digit_hexadecimal_carries_alpha() {
        assert_single(
            "#ff000080",
            0,
            9,
            Color::from_rgba8(0xFF, 0x00, 0x00, 128.0 / 255.0),
        );
    }

    #[test]
    fn test_four_digit_hexadecimal_carries_alpha() {
        assert_single(
            "#f008",
            0,
            5,
            Color::from_rgba8(0xFF, 0x00, 0x00, 136.0 / 255.0),
        );
    }

    #[test]
    fn test_rust_style_prefix() {
        assert_single(
            "const ACCENT: u32 = 0x3A7BD5;",
            20,
            28,
            Color::from_rgb8(0x3A, 0x7B, 0xD5),
        );
    }

    #[test]
    fn test_uppercase_digits_and_prefix() {
        assert_single("0XFF6B6B", 0, 8, Color::from_rgb8(0xFF, 0x6B, 0x6B));
    }

    #[test]
    fn test_functional_notation() {
        assert_single(
            "rgb(58, 123, 213)",
            0,
            17,
            Color::from_rgb8(58, 123, 213),
        );
    }

    #[test]
    fn test_functional_notation_with_alpha() {
        assert_single(
            "border: rgba(255, 0, 0, 0.5)",
            8,
            28,
            Color::from_rgba8(255, 0, 0, 0.5),
        );
    }

    #[test]
    fn test_functional_notation_with_percentages() {
        assert_single(
            "rgb(100%, 0%, 50%)",
            0,
            18,
            Color::from_rgb8(255, 0, 128),
        );
    }

    #[test]
    fn test_functional_notation_is_case_insensitive() {
        assert_single("RGB(0,0,0)", 0, 10, Color::from_rgb8(0, 0, 0));
    }

    #[test]
    fn test_several_literals_on_one_line() {
        let literals = color_literals("a: #fff; b: #000;");
        assert_eq!(literals.len(), 2);
        assert_eq!(literals[0].start_col, 3);
        assert_eq!(literals[1].start_col, 12);
    }

    #[test]
    fn test_columns_count_characters_not_bytes() {
        // "// éàü " is 7 characters but 10 bytes.
        assert_single(
            "// éàü #123456",
            7,
            14,
            Color::from_rgb8(0x12, 0x34, 0x56),
        );
    }

    #[test]
    fn test_invalid_digit_counts_are_rejected() {
        for line in ["#12345", "#1234567", "#12", "#", "0x12345"] {
            assert!(
                color_literals(line).is_empty(),
                "{line:?} should not be a color"
            );
        }
    }

    #[test]
    fn test_literal_continuing_a_word_is_rejected() {
        for line in ["raw0xff0000", "#deadbeefcafe", "0xff0000z"] {
            assert!(
                color_literals(line).is_empty(),
                "{line:?} should not be a color"
            );
        }
    }

    #[test]
    fn test_malformed_functional_notation_is_rejected() {
        for line in [
            "rgb(1, 2)",
            "rgb(1, 2, 3, 4, 5)",
            "rgb(a, b, c)",
            "rgb(1, 2, 3",
            "myrgb(1, 2, 3)",
        ] {
            assert!(
                color_literals(line).is_empty(),
                "{line:?} should not be a color"
            );
        }
    }

    #[test]
    fn test_out_of_range_components_are_clamped() {
        assert_single("rgb(300, -20, 0)", 0, 16, Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn test_line_literals_returns_each_line_it_is_asked_for() {
        let buffer = TextBuffer::new("#ff0000\nplain\nrgb(0, 0, 255)");
        let mut memo = LineLiterals::default();

        assert_eq!(memo.get(&buffer, 0).len(), 1);
        assert!(memo.get(&buffer, 1).is_empty());
        assert_eq!(memo.get(&buffer, 2).len(), 1);
        // Coming back to a line already seen must still answer for that line,
        // not for the one asked about in between.
        assert_eq!(memo.get(&buffer, 0).len(), 1);
    }

    #[test]
    fn test_line_literals_does_not_rescan_the_line_it_already_holds() {
        // The memo is keyed on the line index alone, so a second call for the
        // same index answers from the cache without looking at the buffer.
        // Handing it a *different* buffer is how that becomes observable — and
        // it is also the contract: one memo per draw pass, never across edits.
        let scanned = TextBuffer::new("#ff0000");
        let changed = TextBuffer::new("no color here");
        let mut memo = LineLiterals::default();

        assert_eq!(memo.get(&scanned, 0).len(), 1);

        assert_eq!(
            memo.get(&changed, 0).len(),
            1,
            "line 0 was already held, so it must not have been scanned again"
        );
    }

    #[test]
    fn test_line_without_color_yields_nothing() {
        assert!(color_literals("fn main() { let x = 42; }").is_empty());
        assert!(color_literals("").is_empty());
    }
}
