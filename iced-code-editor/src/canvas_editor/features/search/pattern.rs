//! Whole-word, regular expression, and multiline matching.
use super::{SearchMatch, SearchState};
use crate::buffer::TextBuffer;

#[derive(Debug, Clone)]
pub(super) struct MatchDetail {
    pub start: SearchMatch,
    pub end: (usize, usize),
    pub len: usize,
    pub replacement: String,
}

pub(super) fn find(
    state: &SearchState,
    buffer: &TextBuffer,
    limit: Option<usize>,
) -> Result<Vec<MatchDetail>, String> {
    if state.query.is_empty() {
        return Ok(Vec::new());
    }
    let source = (0..buffer.line_count())
        .map(|line| buffer.line(line))
        .collect::<Vec<_>>()
        .join("\n");
    let pattern = if state.use_regex {
        state.query.clone()
    } else {
        regex::escape(&state.query)
    };
    let pattern =
        if state.whole_word { format!(r"\b(?:{pattern})\b") } else { pattern };
    let regex = regex::RegexBuilder::new(&pattern)
        .case_insensitive(!state.case_sensitive)
        .multi_line(true)
        .build()
        .map_err(|error| error.to_string())?;
    let mut line_starts = vec![0];
    line_starts
        .extend(source.match_indices('\n').map(|(offset, _)| offset + 1));
    let position = |offset: usize| {
        let line = line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        (line, source[line_starts[line]..offset].chars().count())
    };
    let mut results = Vec::new();
    for captures in
        regex.captures_iter(&source).take(limit.unwrap_or(usize::MAX))
    {
        let Some(found) = captures.get(0) else { continue };
        let (line, col) = position(found.start());
        let mut replacement = String::new();
        if state.use_regex {
            captures.expand(&state.replace_with, &mut replacement);
        } else {
            replacement.clone_from(&state.replace_with);
        }
        results.push(MatchDetail {
            start: SearchMatch { line, col },
            end: position(found.end()),
            len: found.as_str().chars().count(),
            replacement,
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(query: &str, replacement: &str) -> SearchState {
        let mut state = SearchState::new();
        state.query = query.into();
        state.replace_with = replacement.into();
        state
    }

    #[test]
    fn literal_patterns_and_replacements_do_not_expand_regex_syntax()
    -> Result<(), String> {
        let state = state("a.*\nb", "$1 ${name} $$");
        let results = find(&state, &TextBuffer::new("xa.*\nby aZZ\nb"), None)?;
        assert_eq!(results.len(), 1);
        assert_eq!((results[0].start.line, results[0].start.col), (0, 1));
        assert_eq!(results[0].end, (1, 1));
        assert_eq!(results[0].len, 5);
        assert_eq!(results[0].replacement, "$1 ${name} $$");
        Ok(())
    }

    #[test]
    fn unicode_offsets_and_crlf_are_logical_character_positions()
    -> Result<(), String> {
        let state = state("😀\n中", "x");
        let results = find(&state, &TextBuffer::new("é😀\r\n中z\r\n"), None)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].start, SearchMatch { line: 0, col: 1 });
        assert_eq!(results[0].end, (1, 1));
        assert_eq!(results[0].len, 3);
        Ok(())
    }

    #[test]
    fn whole_word_boundaries_respect_unicode_and_underscores()
    -> Result<(), String> {
        let mut state = state("café", "x");
        state.whole_word = true;
        let buffer = TextBuffer::new("café décafé café_ CAFÉ");
        let results = find(&state, &buffer, None)?;
        assert_eq!(
            results.iter().map(|item| item.start.col).collect::<Vec<_>>(),
            [0, 18]
        );
        state.case_sensitive = true;
        assert_eq!(find(&state, &buffer, None)?.len(), 1);
        Ok(())
    }

    #[test]
    fn zero_width_regex_matches_are_finite_and_have_correct_line_positions()
    -> Result<(), String> {
        let mut state = state("^|$", "_");
        state.use_regex = true;
        let results = find(&state, &TextBuffer::new("é\n中"), None)?;
        assert_eq!(
            results
                .iter()
                .map(|item| (item.start.line, item.start.col))
                .collect::<Vec<_>>(),
            [(0, 0), (0, 1), (1, 0), (1, 1)]
        );
        assert!(results.iter().all(|item| item.len == 0
            && item.end == (item.start.line, item.start.col)));
        Ok(())
    }

    #[test]
    fn capture_expansion_supports_named_optional_and_literal_dollars()
    -> Result<(), String> {
        let mut state = state(r"(?P<name>é)(\d+)?", "${name}:$2:$$");
        state.use_regex = true;
        let results = find(&state, &TextBuffer::new("é12 é"), None)?;
        assert_eq!(
            results
                .iter()
                .map(|item| item.replacement.as_str())
                .collect::<Vec<_>>(),
            ["é:12:$", "é::$"]
        );
        assert_eq!(
            results.iter().map(|item| item.len).collect::<Vec<_>>(),
            [3, 1]
        );
        Ok(())
    }

    #[test]
    fn limits_include_zero_and_do_not_change_document_order()
    -> Result<(), String> {
        let mut state = state(r"\d+", "x");
        state.use_regex = true;
        let buffer = TextBuffer::new("1 22\n333");
        assert!(find(&state, &buffer, Some(0))?.is_empty());
        assert_eq!(
            find(&state, &buffer, Some(2))?
                .iter()
                .map(|item| item.len)
                .collect::<Vec<_>>(),
            [1, 2]
        );
        assert_eq!(find(&state, &buffer, None)?.len(), 3);
        state.query.clear();
        assert!(find(&state, &buffer, None)?.is_empty());
        Ok(())
    }
}
