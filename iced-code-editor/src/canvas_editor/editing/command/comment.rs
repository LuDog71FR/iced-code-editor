//! Line-comment toggling: mapping a syntax to its comment token and the
//! indentation-aware command that comments/uncomments a line range.

use crate::buffer::TextBuffer;

use super::Command;

/// Returns the line-comment token for a syntax identifier, or `None` if the
/// language has no line comment (e.g. HTML, CSS, Markdown).
///
/// Accepts both file extensions (`"rs"`) and language names (`"rust"`), matching
/// the aliases normalised elsewhere in the editor.
///
/// # Examples
///
/// ```text
/// assert_eq!(line_comment_token("rs"), Some("//"));
/// assert_eq!(line_comment_token("python"), Some("#"));
/// assert_eq!(line_comment_token("html"), None);
/// ```
pub(crate) fn line_comment_token(syntax: &str) -> Option<&'static str> {
    match syntax {
        "rs" | "rust" | "js" | "javascript" | "ts" | "typescript" | "jsx"
        | "tsx" | "go" | "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "c++"
        | "java" | "cs" | "csharp" => Some("//"),
        "py" | "python" | "sh" | "bash" | "shell" | "zsh" | "rb" | "ruby"
        | "toml" | "yaml" | "yml" => Some("#"),
        "lua" => Some("--"),
        _ => None,
    }
}

/// Computes the character count of the leading whitespace of a line.
fn indent_char_count(line: &str) -> usize {
    let trimmed = line.trim_start();
    line[..line.len() - trimmed.len()].chars().count()
}

/// Shifts a position's column by `delta` when it sits at or past the line's
/// indentation, clamping so the column never moves into the indentation.
///
/// Positions left of the indentation (or on an untouched line) are returned
/// unchanged.
fn adjust_column(
    pos: (usize, usize),
    start: usize,
    indents: &[usize],
    deltas: &[isize],
) -> (usize, usize) {
    let Some(idx) = pos.0.checked_sub(start).filter(|&i| i < deltas.len())
    else {
        return pos;
    };
    let indent = indents[idx];
    if pos.1 >= indent {
        let shifted = (pos.1 as isize + deltas[idx]).max(indent as isize);
        (pos.0, shifted as usize)
    } else {
        pos
    }
}

/// Command for toggling line comments on a contiguous range of lines.
///
/// The action (comment vs. uncomment) is decided once at construction so that
/// redo stays consistent: if every non-blank line in `[start, end]` is already
/// commented the range is uncommented, otherwise every non-blank line is
/// commented. The comment token is inserted *after* the existing indentation,
/// and blank lines are left untouched.
#[derive(Debug, Clone)]
pub struct ToggleCommentCommand {
    start: usize,
    old_lines: Vec<String>,
    new_lines: Vec<String>,
    indents: Vec<usize>,
    deltas: Vec<isize>,
    cursor_before: (usize, usize),
    cursor_after: (usize, usize),
}

impl ToggleCommentCommand {
    /// Creates a new toggle-comment command.
    ///
    /// # Arguments
    ///
    /// * `buffer` - The text buffer (read to capture original lines)
    /// * `start` - First line of the range (inclusive)
    /// * `end` - Last line of the range (inclusive)
    /// * `token` - The line-comment token (e.g. `"//"`, inserted as `"// "`)
    /// * `cursor` - Current cursor position
    pub fn new(
        buffer: &TextBuffer,
        start: usize,
        end: usize,
        token: &str,
        cursor: (usize, usize),
    ) -> Self {
        let old_lines: Vec<String> =
            (start..=end).map(|i| buffer.line(i).to_string()).collect();

        // Uncomment only when every non-blank line is already commented.
        let uncomment = old_lines
            .iter()
            .filter(|line| !line.trim_start().is_empty())
            .all(|line| line.trim_start().starts_with(token));

        let token_len = token.chars().count();
        let mut new_lines = Vec::with_capacity(old_lines.len());
        let mut indents = Vec::with_capacity(old_lines.len());
        let mut deltas = Vec::with_capacity(old_lines.len());

        for line in &old_lines {
            let trimmed = line.trim_start();
            let indent_len = indent_char_count(line);
            let indent: String = line.chars().take(indent_len).collect();
            indents.push(indent_len);

            if trimmed.is_empty() {
                // Leave blank lines untouched.
                new_lines.push(line.clone());
                deltas.push(0);
            } else if uncomment {
                let rest = &trimmed[token.len()..];
                // Drop a single space directly after the token, if present.
                let rest = rest.strip_prefix(' ').unwrap_or(rest);
                let new_line = format!("{indent}{rest}");
                deltas.push(
                    new_line.chars().count() as isize
                        - line.chars().count() as isize,
                );
                new_lines.push(new_line);
            } else {
                new_lines.push(format!("{indent}{token} {trimmed}"));
                deltas.push(token_len as isize + 1);
            }
        }

        let cursor_after = adjust_column(cursor, start, &indents, &deltas);

        Self {
            start,
            old_lines,
            new_lines,
            indents,
            deltas,
            cursor_before: cursor,
            cursor_after,
        }
    }

    /// Returns `true` when the toggle leaves the buffer unchanged (e.g. a range
    /// of only blank lines), so the caller can skip recording history.
    pub fn is_noop(&self) -> bool {
        self.old_lines == self.new_lines
    }

    /// Adjusts a position (such as a selection anchor) by the same per-line
    /// column shift this command applies, so selections track the edit.
    pub fn adjust_position(&self, pos: (usize, usize)) -> (usize, usize) {
        adjust_column(pos, self.start, &self.indents, &self.deltas)
    }
}

impl Command for ToggleCommentCommand {
    fn execute(
        &mut self,
        buffer: &mut TextBuffer,
        cursor: &mut (usize, usize),
    ) {
        for (offset, content) in self.new_lines.iter().enumerate() {
            let line_idx = self.start + offset;
            let len = buffer.line_len(line_idx);
            buffer.replace_range(line_idx, 0, len, content);
        }
        *cursor = self.cursor_after;
    }

    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize)) {
        for (offset, content) in self.old_lines.iter().enumerate() {
            let line_idx = self.start + offset;
            let len = buffer.line_len(line_idx);
            buffer.replace_range(line_idx, 0, len, content);
        }
        *cursor = self.cursor_before;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_comment_token() {
        assert_eq!(line_comment_token("rs"), Some("//"));
        assert_eq!(line_comment_token("rust"), Some("//"));
        assert_eq!(line_comment_token("ts"), Some("//"));
        assert_eq!(line_comment_token("go"), Some("//"));
        assert_eq!(line_comment_token("py"), Some("#"));
        assert_eq!(line_comment_token("python"), Some("#"));
        assert_eq!(line_comment_token("lua"), Some("--"));
        assert_eq!(line_comment_token("c"), Some("//"));
        assert_eq!(line_comment_token("cpp"), Some("//"));
        assert_eq!(line_comment_token("java"), Some("//"));
        assert_eq!(line_comment_token("cs"), Some("//"));
        assert_eq!(line_comment_token("sh"), Some("#"));
        assert_eq!(line_comment_token("bash"), Some("#"));
        assert_eq!(line_comment_token("rb"), Some("#"));
        assert_eq!(line_comment_token("ruby"), Some("#"));
        assert_eq!(line_comment_token("toml"), Some("#"));
        assert_eq!(line_comment_token("yaml"), Some("#"));
        assert_eq!(line_comment_token("yml"), Some("#"));
        assert_eq!(line_comment_token("html"), None);
        assert_eq!(line_comment_token("txt"), None);
    }

    #[test]
    fn test_toggle_comment_single_line() {
        let mut buffer = TextBuffer::new("let x = 1;");
        let mut cursor = (0, 4);
        let mut cmd = ToggleCommentCommand::new(&buffer, 0, 0, "//", cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "// let x = 1;");
        // Cursor shifted right by "// " (3 chars).
        assert_eq!(cursor, (0, 7));

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "let x = 1;");
        assert_eq!(cursor, (0, 4));
    }

    #[test]
    fn test_toggle_comment_preserves_indentation() {
        let mut buffer = TextBuffer::new("    let x = 1;");
        let mut cursor = (0, 8);
        let mut cmd = ToggleCommentCommand::new(&buffer, 0, 0, "//", cursor);

        cmd.execute(&mut buffer, &mut cursor);
        // Token inserted after the indentation, not at column 0.
        assert_eq!(buffer.line(0), "    // let x = 1;");
        assert_eq!(cursor, (0, 11));
    }

    #[test]
    fn test_toggle_comment_uncomment() {
        let mut buffer = TextBuffer::new("    // let x = 1;");
        let mut cursor = (0, 11);
        let mut cmd = ToggleCommentCommand::new(&buffer, 0, 0, "//", cursor);

        cmd.execute(&mut buffer, &mut cursor);
        // Removes the token and the single following space.
        assert_eq!(buffer.line(0), "    let x = 1;");
        assert_eq!(cursor, (0, 8));
    }

    #[test]
    fn test_toggle_comment_multiline_mixed_comments_all() {
        let mut buffer = TextBuffer::new("// a\nb\nc");
        let mut cursor = (0, 0);
        // Not all non-blank lines are commented -> comment everything.
        let mut cmd = ToggleCommentCommand::new(&buffer, 0, 2, "//", cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "// // a\n// b\n// c");

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.to_string(), "// a\nb\nc");
    }

    #[test]
    fn test_toggle_comment_skips_blank_lines() {
        let mut buffer = TextBuffer::new("a\n\nb");
        let mut cursor = (0, 0);
        let mut cmd = ToggleCommentCommand::new(&buffer, 0, 2, "//", cursor);

        cmd.execute(&mut buffer, &mut cursor);
        // The blank middle line is left untouched.
        assert_eq!(buffer.to_string(), "// a\n\n// b");
    }

    #[test]
    fn test_toggle_comment_python_token() {
        let mut buffer = TextBuffer::new("x = 1");
        let mut cursor = (0, 0);
        let mut cmd = ToggleCommentCommand::new(&buffer, 0, 0, "#", cursor);

        cmd.execute(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "# x = 1");

        cmd.undo(&mut buffer, &mut cursor);
        assert_eq!(buffer.line(0), "x = 1");
    }

    #[test]
    fn test_toggle_comment_noop_on_blank_range() {
        let buffer = TextBuffer::new("\n  \n");
        let cmd = ToggleCommentCommand::new(&buffer, 0, 2, "//", (0, 0));
        assert!(cmd.is_noop());
    }
}
