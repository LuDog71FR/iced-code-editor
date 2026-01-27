//! Text search functionality for the code editor.
//!
//! This module provides efficient text search capabilities including:
//! - Case-sensitive and case-insensitive search
//! - Multiple match detection
//! - Position tracking for highlighting

use crate::text_buffer::TextBuffer;
use iced::widget::Id;

/// Represents a search match position in the buffer.
///
/// Contains the line and column position of a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line index (0-based)
    pub line: usize,
    /// Column index (0-based, UTF-8 character offset)
    pub col: usize,
}

/// Which field in the search dialog currently has focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchFocusedField {
    /// Search input field has focus
    Search,
    /// Replace input field has focus
    Replace,
}

/// Search state management.
///
/// Tracks the current search query, options, and results.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Current search query
    pub query: String,
    /// Text to replace with
    pub replace_with: String,
    /// Case-sensitive search flag
    pub case_sensitive: bool,
    /// Whether the search dialog is visible
    pub is_open: bool,
    /// Whether replace mode is active (true) or just search (false)
    pub is_replace_mode: bool,
    /// List of all matches found
    pub matches: Vec<SearchMatch>,
    /// Index of the currently selected match
    pub current_match_index: Option<usize>,
    /// ID for the search text input (for focus management)
    pub search_input_id: Id,
    /// ID for the replace text input (for focus management)
    pub replace_input_id: Id,
    /// Which field currently has focus (for Tab navigation)
    pub focused_field: SearchFocusedField,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            query: String::new(),
            replace_with: String::new(),
            case_sensitive: false,
            is_open: false,
            is_replace_mode: false,
            matches: Vec::new(),
            current_match_index: None,
            search_input_id: Id::unique(),
            replace_input_id: Id::unique(),
            focused_field: SearchFocusedField::Search,
        }
    }
}

impl SearchState {
    /// Creates a new search state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens the search dialog in search-only mode.
    pub fn open_search(&mut self) {
        self.is_open = true;
        self.is_replace_mode = false;
        self.focused_field = SearchFocusedField::Search;
    }

    /// Opens the search dialog in search-and-replace mode.
    pub fn open_replace(&mut self) {
        self.is_open = true;
        self.is_replace_mode = true;
        self.focused_field = SearchFocusedField::Search;
    }

    /// Closes the search dialog.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Cycles focus to the next field (Tab).
    pub fn focus_next_field(&mut self) {
        if self.is_replace_mode {
            self.focused_field = match self.focused_field {
                SearchFocusedField::Search => SearchFocusedField::Replace,
                SearchFocusedField::Replace => SearchFocusedField::Search,
            };
        }
        // In search-only mode, do nothing
    }

    /// Cycles focus to the previous field (Shift+Tab).
    pub fn focus_previous_field(&mut self) {
        if self.is_replace_mode {
            self.focused_field = match self.focused_field {
                SearchFocusedField::Search => SearchFocusedField::Replace,
                SearchFocusedField::Replace => SearchFocusedField::Search,
            };
        }
        // In search-only mode, do nothing
    }

    /// Updates the search query and triggers a new search.
    pub fn set_query(&mut self, query: String, buffer: &TextBuffer) {
        self.query = query;
        self.update_matches(buffer);
    }

    /// Updates the replace text.
    pub fn set_replace_with(&mut self, replace_with: String) {
        self.replace_with = replace_with;
    }

    /// Toggles case sensitivity and re-runs the search.
    pub fn toggle_case_sensitive(&mut self, buffer: &TextBuffer) {
        self.case_sensitive = !self.case_sensitive;
        self.update_matches(buffer);
    }

    /// Updates the matches list based on current query and options.
    pub fn update_matches(&mut self, buffer: &TextBuffer) {
        self.matches = find_matches(buffer, &self.query, self.case_sensitive);

        // Update current match index
        if self.matches.is_empty() {
            self.current_match_index = None;
        } else if self.current_match_index.is_none() {
            self.current_match_index = Some(0);
        } else if let Some(idx) = self.current_match_index {
            // Clamp to valid range
            if idx >= self.matches.len() {
                self.current_match_index =
                    Some(self.matches.len().saturating_sub(1));
            }
        }
    }

    /// Moves to the next match (circular).
    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }

        self.current_match_index = Some(match self.current_match_index {
            Some(idx) => {
                if idx + 1 >= self.matches.len() {
                    0 // Wrap to first
                } else {
                    idx + 1
                }
            }
            None => 0,
        });
    }

    /// Moves to the previous match (circular).
    pub fn previous_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }

        self.current_match_index = Some(match self.current_match_index {
            Some(idx) => {
                if idx == 0 {
                    self.matches.len() - 1 // Wrap to last
                } else {
                    idx - 1
                }
            }
            None => self.matches.len() - 1,
        });
    }

    /// Returns the current match position if available.
    #[must_use]
    pub fn current_match(&self) -> Option<SearchMatch> {
        self.current_match_index.and_then(|idx| self.matches.get(idx).copied())
    }

    /// Returns the number of matches found.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    /// Selects the match closest to the given cursor position.
    ///
    /// This is useful after buffer modifications to maintain context.
    /// If no matches exist, sets current_match_index to None.
    pub fn select_match_near_cursor(&mut self, cursor: (usize, usize)) {
        if self.matches.is_empty() {
            self.current_match_index = None;
            return;
        }

        let (cursor_line, cursor_col) = cursor;

        // Find the match with minimum distance to cursor
        let closest_index = self
            .matches
            .iter()
            .enumerate()
            .min_by_key(|(_, m)| {
                // Calculate Manhattan distance, weighing lines more than columns
                let line_dist =
                    (m.line as isize - cursor_line as isize).unsigned_abs();
                let col_dist =
                    (m.col as isize - cursor_col as isize).unsigned_abs();
                line_dist * 1000 + col_dist
            })
            .map(|(i, _)| i);

        self.current_match_index = closest_index;
    }
}

/// Finds all matches of a query in the text buffer.
///
/// # Arguments
///
/// * `buffer` - The text buffer to search in
/// * `query` - The search string
/// * `case_sensitive` - Whether to perform case-sensitive search
///
/// # Returns
///
/// A vector of all match positions found
#[must_use]
pub fn find_matches(
    buffer: &TextBuffer,
    query: &str,
    case_sensitive: bool,
) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let search_query =
        if case_sensitive { query.to_string() } else { query.to_lowercase() };

    for line_idx in 0..buffer.line_count() {
        let line = buffer.line(line_idx);
        let search_line =
            if case_sensitive { line.to_string() } else { line.to_lowercase() };

        // Find all occurrences in this line
        let mut start_pos = 0;
        while let Some(relative_pos) =
            search_line[start_pos..].find(&search_query)
        {
            let absolute_pos = start_pos + relative_pos;

            // Convert byte position to character position
            let col = line[..absolute_pos].chars().count();

            matches.push(SearchMatch { line: line_idx, col });

            // Move past this match to find next occurrence
            // Use search_query.len() to avoid overlapping matches and ensure we land on UTF-8 character boundary
            start_pos = absolute_pos + search_query.len();
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches_case_sensitive() {
        let buffer = TextBuffer::new("Hello World\nhello world");
        let matches = find_matches(&buffer, "hello", true);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[0].col, 0);
    }

    #[test]
    fn test_find_matches_case_insensitive() {
        let buffer = TextBuffer::new("Hello World\nhello world");
        let matches = find_matches(&buffer, "hello", false);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 0);
        assert_eq!(matches[0].col, 0);
        assert_eq!(matches[1].line, 1);
        assert_eq!(matches[1].col, 0);
    }

    #[test]
    fn test_find_matches_multiple_occurrences() {
        let buffer = TextBuffer::new("foo bar foo baz foo");
        let matches = find_matches(&buffer, "foo", false);

        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].col, 0);
        assert_eq!(matches[1].col, 8);
        assert_eq!(matches[2].col, 16);
    }

    #[test]
    fn test_find_matches_multiline() {
        let buffer = TextBuffer::new("line1\nfoo\nline3\nfoo");
        let matches = find_matches(&buffer, "foo", false);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].line, 1);
        assert_eq!(matches[1].line, 3);
    }

    #[test]
    fn test_find_matches_empty_query() {
        let buffer = TextBuffer::new("Hello World");
        let matches = find_matches(&buffer, "", false);

        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_find_matches_no_results() {
        let buffer = TextBuffer::new("Hello World");
        let matches = find_matches(&buffer, "xyz", false);

        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_search_state_navigation() {
        let buffer = TextBuffer::new("foo bar foo baz foo");
        let mut state = SearchState::new();
        state.set_query("foo".to_string(), &buffer);

        assert_eq!(state.match_count(), 3);
        assert_eq!(state.current_match_index, Some(0));

        state.next_match();
        assert_eq!(state.current_match_index, Some(1));

        state.next_match();
        assert_eq!(state.current_match_index, Some(2));

        // Test wrap-around
        state.next_match();
        assert_eq!(state.current_match_index, Some(0));

        // Test previous
        state.previous_match();
        assert_eq!(state.current_match_index, Some(2));
    }

    #[test]
    fn test_search_state_toggle_case() {
        let buffer = TextBuffer::new("Hello hello");
        let mut state = SearchState::new();
        state.set_query("hello".to_string(), &buffer);

        assert_eq!(state.match_count(), 2);

        state.toggle_case_sensitive(&buffer);
        assert_eq!(state.match_count(), 1);

        state.toggle_case_sensitive(&buffer);
        assert_eq!(state.match_count(), 2);
    }
}
