//! Word boundaries for `Ctrl+Left`/`Ctrl+Right` (`Alt` on macOS) and the
//! matching word deletions.
//!
//! A "word" is a run of characters of the same class: identifier characters
//! (alphanumerics and `_`) or punctuation. Whitespace between runs is skipped,
//! as in VS Code, JetBrains and Sublime Text. A move at a line boundary
//! crosses into the neighbouring line.

use crate::buffer::TextBuffer;

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

/// Column where the word left of `col` starts, within one line.
///
/// Skips whitespace first, then the run of characters of one class.
pub(crate) fn word_left_in_line(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut index = col.min(chars.len());
    while index > 0 && class(chars[index - 1]) == Class::Space {
        index -= 1;
    }
    if index > 0 {
        let run = class(chars[index - 1]);
        while index > 0 && class(chars[index - 1]) == run {
            index -= 1;
        }
    }
    index
}

/// Column where the word right of `col` ends, within one line.
///
/// Skips whitespace first, then the run of characters of one class.
pub(crate) fn word_right_in_line(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut index = col.min(chars.len());
    while index < chars.len() && class(chars[index]) == Class::Space {
        index += 1;
    }
    if index < chars.len() {
        let run = class(chars[index]);
        while index < chars.len() && class(chars[index]) == run {
            index += 1;
        }
    }
    index
}

/// Position one word to the left of `position`. At the start of a line it
/// goes to the end of the previous one.
pub(crate) fn word_left(
    buffer: &TextBuffer,
    position: (usize, usize),
) -> (usize, usize) {
    let (line, col) = position;
    if col == 0 {
        return match line.checked_sub(1) {
            Some(previous) => (previous, buffer.line_len(previous)),
            None => position,
        };
    }
    (line, word_left_in_line(buffer.line(line), col))
}

/// Position one word to the right of `position`. At the end of a line it
/// goes to the start of the next one.
pub(crate) fn word_right(
    buffer: &TextBuffer,
    position: (usize, usize),
) -> (usize, usize) {
    let (line, col) = position;
    if col >= buffer.line_len(line) {
        return if line + 1 < buffer.line_count() {
            (line + 1, 0)
        } else {
            position
        };
    }
    (line, word_right_in_line(buffer.line(line), col))
}

/// Column of the first non-whitespace character of `line` (its length when
/// the line is blank).
pub(crate) fn first_non_blank(line: &str) -> usize {
    line.chars().take_while(|ch| ch.is_whitespace()).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_skips_spaces_then_one_run() {
        let line = "let foo_bar = baz(1);";
        assert_eq!(word_left_in_line(line, 11), 4); // after "foo_bar" + space
        assert_eq!(word_left_in_line(line, 4), 0);
        assert_eq!(word_left_in_line(line, 21), 19); // ");"
        assert_eq!(word_left_in_line(line, 18), 17); // "("
    }

    #[test]
    fn right_skips_spaces_then_one_run() {
        let line = "let foo_bar = baz(1);";
        assert_eq!(word_right_in_line(line, 0), 3);
        assert_eq!(word_right_in_line(line, 3), 11);
        assert_eq!(word_right_in_line(line, 11), 13); // "="
        assert_eq!(word_right_in_line(line, 17), 18); // "("
    }

    #[test]
    fn unicode_letters_are_word_characters() {
        let line = "ação = 1";
        assert_eq!(word_right_in_line(line, 0), 4);
        assert_eq!(word_left_in_line(line, 4), 0);
    }

    #[test]
    fn crosses_line_boundaries() {
        let buffer = TextBuffer::new("ab\n  cd");
        assert_eq!(word_right(&buffer, (0, 2)), (1, 0));
        assert_eq!(word_left(&buffer, (1, 0)), (0, 2));
        assert_eq!(word_left(&buffer, (0, 0)), (0, 0));
        assert_eq!(word_right(&buffer, (1, 4)), (1, 4));
    }

    #[test]
    fn leading_blanks() {
        assert_eq!(first_non_blank("    x"), 4);
        assert_eq!(first_non_blank("\tx"), 1);
        assert_eq!(first_non_blank("   "), 3);
        assert_eq!(first_non_blank(""), 0);
    }
}
