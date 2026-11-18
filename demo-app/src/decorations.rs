//! The "Decorations" template: virtual text and range styles driven by the
//! host application.
//!
//! A real host gets these from a language server (inlay hints and semantic
//! tokens). The demo derives them from the text with a few line-based rules,
//! and replaces both layers after every editor event, which is what the
//! editor expects from the owner of a layer: positions do not follow edits.

use iced::Color;
use iced_code_editor::{CodeEditor, RangeStyle, VirtualText};
use std::ops::Range;
use std::sync::LazyLock;

/// Wrap column of a decorated tab, narrow enough to show how a hint at the end
/// of a line wraps.
const WRAP_COLUMN: usize = 60;
/// Layer holding the demo's inlay hints.
const HINTS_LAYER: &str = "demo-inlay-hints";
/// Layer holding the demo's semantic colors.
const RANGES_LAYER: &str = "demo-semantic-tokens";
/// Width of the sample's long `print` line: its end-of-line hint only fits on
/// the next row.
const PRINT_LINE_WIDTH: usize = 58;
/// Number of lines in the sample's loop, enough to scroll the headers away.
const LOOP_BODY_LINES: usize = 60;
/// Hint drawn at the end of a function header.
const FUNCTION_HINT: &str = "  -> number";
/// Hint drawn at the end of a `print` call.
const PRINT_HINT: &str = "  -- printed";
/// Hint color when the syntax theme has no color for comments.
const FALLBACK_HINT_COLOR: Color = Color::from_rgb(0.55, 0.55, 0.55);
/// Parameter color when the syntax theme has none.
const FALLBACK_PARAMETER_COLOR: Color = Color::from_rgb(0.9, 0.55, 0.3);
/// Color of a function name, distinct from every syntax theme's own.
const FUNCTION_NAME_COLOR: Color = Color::from_rgb(0.95, 0.35, 0.75);

/// The sample text, built once.
static SAMPLE: LazyLock<String> = LazyLock::new(build_sample);

/// Returns the Lua text of the "Decorations" template.
pub fn sample() -> &'static str {
    SAMPLE.as_str()
}

/// Builds the Lua text of the "Decorations" template.
fn build_sample() -> String {
    let message_width = PRINT_LINE_WIDTH - r#"    print("")"#.len();
    let mut lines = vec![
        "-- Hints and semantic colors are not part of this text.".to_string(),
        "-- Scroll down: the pinned headers keep them.".to_string(),
        "local Account = {}".to_string(),
        String::new(),
        "function Account.deposit(balance, amount)".to_string(),
        "    local fee = 2".to_string(),
        "    local label = \"deposit\"".to_string(),
        format!("    print(\"{:.<message_width$}\")", "Deposit recorded"),
        format!("    for step = 1, {LOOP_BODY_LINES} do"),
    ];
    lines
        .extend((1..=LOOP_BODY_LINES).map(|step| {
            format!("        balance = balance + amount * {step}")
        }));
    lines.extend(
        [
            "    end",
            "    return balance - fee",
            "end",
            "",
            "print(Account.deposit(100, 5))",
            "",
        ]
        .map(str::to_string),
    );
    lines.join("\n")
}

/// Decorates `editor` and narrows its wrap column.
pub fn enable(editor: &mut CodeEditor) {
    editor.set_wrap_column(Some(WRAP_COLUMN));
    apply(editor);
}

/// Removes the decorations of `editor` and restores its wrap column.
pub fn disable(editor: &mut CodeEditor) {
    editor.set_wrap_column(None);
    editor.clear_virtual_text(HINTS_LAYER);
    editor.clear_range_styles(RANGES_LAYER);
}

/// Replaces both layers of `editor` from its current text and theme.
pub fn apply(editor: &mut CodeEditor) {
    let content = editor.content();
    let hint_color =
        editor.scope_color("comment").unwrap_or(FALLBACK_HINT_COLOR);
    let parameter_color = editor
        .scope_color("variable.parameter")
        .unwrap_or(FALLBACK_PARAMETER_COLOR);
    editor.set_virtual_text(HINTS_LAYER, virtual_text(&content, hint_color));
    editor.set_range_styles(
        RANGES_LAYER,
        range_styles(&content, parameter_color),
    );
}

/// Builds the inlay hints of `content`.
fn virtual_text(content: &str, color: Color) -> Vec<VirtualText> {
    let mut items = Vec::new();
    for (line, text) in content.lines().enumerate() {
        if let Some((column, hint)) = type_hint(text) {
            items.push(VirtualText {
                line,
                column,
                text: hint.to_string(),
                color,
            });
        }
        if let Some(hint) = end_of_line_hint(text) {
            items.push(VirtualText {
                line,
                column: text.chars().count(),
                text: hint.to_string(),
                color,
            });
        }
    }
    items
}

/// Builds the semantic colors of `content`: function names, and parameters
/// from their function's header to its closing `end`.
fn range_styles(content: &str, parameter_color: Color) -> Vec<RangeStyle> {
    let mut styles = Vec::new();
    let mut parameters: Vec<&str> = Vec::new();
    for (line, text) in content.lines().enumerate() {
        if let Some((name, names)) = function_parts(text) {
            styles.push(RangeStyle {
                line,
                start: name.start,
                end: name.end,
                color: FUNCTION_NAME_COLOR,
            });
            parameters = names;
        }
        for parameter in &parameters {
            styles.extend(word_ranges(text, parameter).into_iter().map(
                |range| RangeStyle {
                    line,
                    start: range.start,
                    end: range.end,
                    color: parameter_color,
                },
            ));
        }
        if text == "end" {
            parameters.clear();
        }
    }
    styles
}

/// Returns the column after the name of a `local name = literal` line, and the
/// type of the literal.
fn type_hint(line: &str) -> Option<(usize, &'static str)> {
    let rest = line.trim_start().strip_prefix("local ")?;
    let (name, value) = rest.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || !name.chars().all(is_word_char) {
        return None;
    }
    let hint = match value.trim_start().chars().next()? {
        '0'..='9' => ": number",
        '"' => ": string",
        '{' => ": table",
        _ => return None,
    };
    let start = line.len() - rest.len() + rest.find(name)?;
    Some((char_range(line, start, start + name.len())?.end, hint))
}

/// Returns the hint drawn after the last character of `line`, if any.
fn end_of_line_hint(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("function ") {
        Some(FUNCTION_HINT)
    } else if trimmed.starts_with("print(") {
        Some(PRINT_HINT)
    } else {
        None
    }
}

/// Splits a `function name(a, b)` header into the character columns of its
/// name and the names of its parameters.
fn function_parts(line: &str) -> Option<(Range<usize>, Vec<&str>)> {
    let rest = line.trim_start().strip_prefix("function ")?;
    let name_start = line.len() - rest.len();
    let open = name_start + rest.find('(')?;
    let close = open + line.get(open..)?.find(')')?;
    let name = line.get(name_start..open)?.trim_end();
    if name.is_empty() {
        return None;
    }
    let parameters = line
        .get(open + 1..close)?
        .split(',')
        .map(str::trim)
        .filter(|parameter| !parameter.is_empty())
        .collect();
    Some((char_range(line, name_start, name_start + name.len())?, parameters))
}

/// Returns the character columns of every whole-word occurrence of `word`.
fn word_ranges(line: &str, word: &str) -> Vec<Range<usize>> {
    line.match_indices(word)
        .filter(|(start, _)| {
            let before =
                line.get(..*start).and_then(|text| text.chars().next_back());
            let after = line
                .get(*start + word.len()..)
                .and_then(|text| text.chars().next());
            !before.is_some_and(is_word_char)
                && !after.is_some_and(is_word_char)
        })
        .filter_map(|(start, _)| char_range(line, start, start + word.len()))
        .collect()
}

/// Returns whether `c` can be part of an identifier.
fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Converts the byte range `start..end` of `line` to character columns.
fn char_range(line: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let first = line.get(..start)?.chars().count();
    Some(first..first + line.get(start..end)?.chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{DemoApp, Message};
    use crate::types::{EditorId, Template};

    const GRAY: Color = Color::from_rgb(0.5, 0.5, 0.5);

    /// Numbers of hints and range styles of the active tab, with its wrap
    /// column.
    fn active_state(
        app: &mut DemoApp,
    ) -> Option<(usize, usize, Option<usize>)> {
        app.get_active_tab().map(|tab| {
            (
                tab.editor.virtual_text(HINTS_LAYER).len(),
                tab.editor.range_styles(RANGES_LAYER).len(),
                tab.editor.wrap_column(),
            )
        })
    }

    #[test]
    fn the_sample_print_line_leaves_no_room_for_its_hint() {
        let line = sample().lines().find(|line| line.contains("Deposit"));
        let width = line.map(|line| line.chars().count());
        assert_eq!(width, Some(PRINT_LINE_WIDTH));
        assert!(PRINT_LINE_WIDTH + PRINT_HINT.len() > WRAP_COLUMN);
    }

    #[test]
    fn type_hints_follow_the_name_of_a_literal() {
        assert_eq!(type_hint("    local fee = 2"), Some((13, ": number")));
        assert_eq!(type_hint("local s = \"a\""), Some((7, ": string")));
        assert_eq!(type_hint("local été = {}"), Some((9, ": table")));
        assert_eq!(type_hint("local x = other"), None);
        assert_eq!(type_hint("local a, b = 1"), None);
        assert_eq!(type_hint("x = 1"), None);
    }

    #[test]
    fn end_of_line_hints_mark_functions_and_prints() {
        assert_eq!(end_of_line_hint("function f()"), Some(FUNCTION_HINT));
        assert_eq!(end_of_line_hint("    print(1)"), Some(PRINT_HINT));
        assert_eq!(end_of_line_hint("local x = 1"), None);
    }

    #[test]
    fn function_headers_split_into_name_and_parameters() {
        assert_eq!(
            function_parts("function Account.deposit(balance, amount)"),
            Some((9..24, vec!["balance", "amount"]))
        );
        assert_eq!(function_parts("function f()"), Some((9..10, Vec::new())));
        assert_eq!(function_parts("function (x)"), None);
        assert_eq!(function_parts("local x = 1"), None);
    }

    #[test]
    fn word_ranges_match_whole_words_in_character_columns() {
        assert_eq!(word_ranges("é a + aa + a_b + a", "a"), vec![2..3, 17..18]);
        assert!(word_ranges("nothing", "a").is_empty());
    }

    #[test]
    fn parameters_are_colored_until_the_end_of_their_function() {
        let content = "function f(a)\n  a = a\nend\na = 1\n";
        let styles = range_styles(content, GRAY);
        let parameters: Vec<(usize, usize)> = styles
            .iter()
            .filter(|style| style.color == GRAY)
            .map(|style| (style.line, style.start))
            .collect();
        assert_eq!(parameters, vec![(0, 11), (1, 2), (1, 6)]);
    }

    #[test]
    fn apply_replaces_the_layers_after_the_text_changes() {
        let mut editor = CodeEditor::new("local x = 1", "lua");
        apply(&mut editor);
        assert_eq!(editor.virtual_text(HINTS_LAYER).len(), 1);

        let _ = editor.reset("\nlocal x = 1");
        apply(&mut editor);
        let line = editor.virtual_text(HINTS_LAYER).first().map(|i| i.line);
        assert_eq!(line, Some(1));

        let _ = editor.reset("");
        apply(&mut editor);
        assert!(editor.virtual_text(HINTS_LAYER).is_empty());
    }

    #[test]
    fn the_template_decorates_its_tab_and_others_remove_it() {
        let (mut app, _) = DemoApp::new();
        let id = EditorId(0);

        let _ =
            app.update(Message::TemplateSelected(id, Template::Decorations));
        let state = active_state(&mut app);
        assert!(state.is_some_and(|(hints, ranges, wrap)| hints > 0
            && ranges > 0
            && wrap == Some(WRAP_COLUMN)));

        let _ = app.update(Message::TemplateSelected(id, Template::HelloWorld));
        assert_eq!(active_state(&mut app), Some((0, 0, None)));
    }
}
