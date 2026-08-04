//! Matching bracket/quote detection for the bracket-pair highlight overlay.

use crate::text_buffer::TextBuffer;

/// Returns `true` if `ch` is one of the opening brackets `( [ {`.
fn is_opening_bracket(ch: char) -> bool {
    matches!(ch, '(' | '[' | '{')
}

/// Returns `true` if `ch` is one of the closing brackets `) ] }`.
fn is_closing_bracket(ch: char) -> bool {
    matches!(ch, ')' | ']' | '}')
}

/// Returns `true` if `ch` is a single or double quote.
fn is_quote(ch: char) -> bool {
    matches!(ch, '"' | '\'')
}

/// Returns the counterpart of a bracket character, in either direction.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(bracket_pair('('), Some(')'));
/// assert_eq!(bracket_pair(')'), Some('('));
/// assert_eq!(bracket_pair('x'), None);
/// ```
fn bracket_pair(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        ')' => Some('('),
        '[' => Some(']'),
        ']' => Some('['),
        '{' => Some('}'),
        '}' => Some('{'),
        _ => None,
    }
}

/// Scans forward from just after `(line, col)` for the closing bracket that
/// matches the opening bracket `open` (which sits at `(line, col)`).
///
/// Tracks nesting depth for same-family brackets only, so `(` inside a `[]`
/// pair is ignored while looking for a `)`. This is a plain textual scan —
/// it does not skip over brackets found inside strings or comments.
fn scan_forward(
    buffer: &TextBuffer,
    open: char,
    close: char,
    line: usize,
    col: usize,
) -> Option<(usize, usize)> {
    let mut depth = 1usize;
    let mut start_col = col + 1;

    for l in line..buffer.line_count() {
        let text = buffer.line(l);
        for (c, ch) in text.chars().enumerate().skip(start_col) {
            if ch == open {
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    return Some((l, c));
                }
            }
        }
        start_col = 0;
    }

    None
}

/// Scans backward from just before `(line, col)` for the opening bracket
/// that matches the closing bracket `close` (which sits at `(line, col)`).
///
/// Mirrors [`scan_forward`]: tracks same-family nesting depth, no
/// string/comment awareness.
fn scan_backward(
    buffer: &TextBuffer,
    open: char,
    close: char,
    line: usize,
    col: usize,
) -> Option<(usize, usize)> {
    let mut depth = 1usize;
    let mut l = line;

    loop {
        let text = buffer.line(l);
        let end_col = if l == line { col } else { text.chars().count() };
        let chars: Vec<char> = text.chars().take(end_col).collect();

        for (c, ch) in chars.iter().enumerate().rev() {
            if *ch == close {
                depth += 1;
            } else if *ch == open {
                depth -= 1;
                if depth == 0 {
                    return Some((l, c));
                }
            }
        }

        if l == 0 {
            return None;
        }
        l -= 1;
    }
}

/// Finds the matching quote for the quote character `quote` sitting at
/// `(line, target_col)`.
///
/// Since `"` and `'` don't distinguish opener from closer, quotes on the
/// line are paired sequentially in the order they appear (1st with 2nd, 3rd
/// with 4th, ...). Matching is scoped to a single line and doesn't account
/// for escaped quotes (e.g. `\"`) — consistent with the plain textual scan
/// used for brackets. Returns `None` if `target_col` isn't part of a
/// complete pair (e.g. an odd, unterminated quote).
fn find_matching_quote(
    buffer: &TextBuffer,
    line: usize,
    target_col: usize,
    quote: char,
) -> Option<(usize, usize)> {
    let text = buffer.line(line);
    let positions: Vec<usize> = text
        .chars()
        .enumerate()
        .filter(|(_, ch)| *ch == quote)
        .map(|(c, _)| c)
        .collect();

    let idx = positions.iter().position(|&c| c == target_col)?;
    let partner_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
    positions.get(partner_idx).map(|&c| (line, c))
}

/// Finds the bracket or quote pair touching the cursor at `pos`, if any.
///
/// Checks the character immediately after the cursor first, then the
/// character immediately before it (matching common editor behaviour: the
/// cursor "touches" brackets on either side). If a bracket is found, the
/// buffer is scanned for its balanced counterpart.
///
/// Returns `Some((target_pos, match_pos))` with `target_pos` the
/// bracket/quote nearest the cursor and `match_pos` its counterpart, or
/// `None` if the cursor isn't touching a bracket or quote, or it has no
/// matching counterpart.
///
/// # Examples
///
/// ```ignore
/// // buffer contains "foo(bar)"
/// let pos = find_matching_pair(&buffer, (0, 3)); // cursor before '('
/// assert_eq!(pos, Some(((0, 3), (0, 7))));
/// ```
pub(crate) fn find_matching_pair(
    buffer: &TextBuffer,
    pos: (usize, usize),
) -> Option<((usize, usize), (usize, usize))> {
    let (line, col) = pos;
    let text = buffer.line(line);
    let char_after = text.chars().nth(col);
    let char_before = if col > 0 { text.chars().nth(col - 1) } else { None };
    let is_pairable = |c: &char| {
        is_opening_bracket(*c) || is_closing_bracket(*c) || is_quote(*c)
    };

    let (target_pos, ch) = if let Some(ch) = char_after.filter(is_pairable) {
        ((line, col), ch)
    } else if let Some(ch) = char_before.filter(is_pairable) {
        ((line, col - 1), ch)
    } else {
        return None;
    };

    if is_quote(ch) {
        let match_pos =
            find_matching_quote(buffer, target_pos.0, target_pos.1, ch)?;
        return Some((target_pos, match_pos));
    }

    let other = bracket_pair(ch)?;
    let match_pos = if is_opening_bracket(ch) {
        scan_forward(buffer, ch, other, target_pos.0, target_pos.1)
    } else {
        scan_backward(buffer, other, ch, target_pos.0, target_pos.1)
    }?;

    Some((target_pos, match_pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buffer_from(lines: &[&str]) -> TextBuffer {
        TextBuffer::new(&lines.join("\n"))
    }

    #[test]
    fn cursor_before_opening_bracket_matches_closing() {
        let buffer = buffer_from(&["foo(bar)"]);
        assert_eq!(find_matching_pair(&buffer, (0, 3)), Some(((0, 3), (0, 7))));
    }

    #[test]
    fn cursor_after_closing_bracket_matches_opening() {
        let buffer = buffer_from(&["foo(bar)"]);
        assert_eq!(find_matching_pair(&buffer, (0, 8)), Some(((0, 7), (0, 3))));
    }

    #[test]
    fn cursor_after_opening_bracket_also_matches() {
        let buffer = buffer_from(&["foo(bar)"]);
        assert_eq!(find_matching_pair(&buffer, (0, 4)), Some(((0, 3), (0, 7))));
    }

    #[test]
    fn nested_brackets_match_correct_pair() {
        let buffer = buffer_from(&["a([b]c)"]);
        // Cursor right before the inner '[' should match its own ']'.
        assert_eq!(find_matching_pair(&buffer, (0, 2)), Some(((0, 2), (0, 4))));
        // Cursor right before the outer '(' should match the outer ')'.
        assert_eq!(find_matching_pair(&buffer, (0, 1)), Some(((0, 1), (0, 6))));
    }

    #[test]
    fn unmatched_bracket_returns_none() {
        let buffer = buffer_from(&["foo(bar"]);
        assert_eq!(find_matching_pair(&buffer, (0, 3)), None);
    }

    #[test]
    fn cursor_not_near_bracket_returns_none() {
        let buffer = buffer_from(&["foo(bar)"]);
        assert_eq!(find_matching_pair(&buffer, (0, 1)), None);
    }

    #[test]
    fn multi_line_pair_matches_across_lines() {
        let buffer = buffer_from(&["fn main() {", "    let x = 1;", "}"]);
        assert_eq!(
            find_matching_pair(&buffer, (0, 10)),
            Some(((0, 10), (2, 0)))
        );
    }

    #[test]
    fn cursor_before_opening_double_quote_matches_closing() {
        let buffer = buffer_from(&[r#"let s = "hello";"#]);
        assert_eq!(
            find_matching_pair(&buffer, (0, 8)),
            Some(((0, 8), (0, 14)))
        );
    }

    #[test]
    fn cursor_after_closing_single_quote_matches_opening() {
        let buffer = buffer_from(&["let c = 'a';"]);
        assert_eq!(
            find_matching_pair(&buffer, (0, 11)),
            Some(((0, 10), (0, 8)))
        );
    }

    #[test]
    fn two_string_literals_on_same_line_pair_independently() {
        let buffer = buffer_from(&[r#"foo("a", "b")"#]);
        assert_eq!(find_matching_pair(&buffer, (0, 4)), Some(((0, 4), (0, 6))));
        assert_eq!(
            find_matching_pair(&buffer, (0, 9)),
            Some(((0, 9), (0, 11)))
        );
    }

    #[test]
    fn unterminated_quote_returns_none() {
        let buffer = buffer_from(&[r#"let s = "hello;"#]);
        assert_eq!(find_matching_pair(&buffer, (0, 8)), None);
    }
}
