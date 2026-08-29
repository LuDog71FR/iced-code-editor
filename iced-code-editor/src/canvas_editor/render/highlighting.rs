//! Syntax resolution and token highlighting for [`CodeEditor`].
//!
//! Everything here answers "what colour is this text?", which is a different
//! question from "where on the canvas does it go?" -- the latter lives in
//! [`super::text`]. The two process-wide syntect singletons, the per-editor
//! memo in front of them ([`CodeEditor::resolve_syntax`]), the incremental
//! per-line cache ([`CodeEditor::highlighted_line_cached`]) and the
//! rainbow-bracket palettes are all colour decisions, so they sit together
//! here, leaving `text.rs` with the `draw_*` family.

use iced::Color;
use std::rc::Rc;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    HighlightIterator, HighlightState, Highlighter, Style, ThemeSet,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

use crate::canvas_editor::{CodeEditor, HighlightCache, ResolvedSyntax};

/// Loading syntect's syntax definitions is expensive, so the set is built once
/// per process and shared by every editor instance.
static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Syntect's bundled themes, loaded once per process (see [`SYNTAX_SET`]).
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

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
/// the sequential `CodeEditor::highlighted_line_cached` instead.
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

/// Color cycle for bracket-pair colorization on a **dark** editor background,
/// indexed by nesting depth modulo its length so a matching pair always shares
/// a color. Matches the well-known VS Code Dark+ rainbow-bracket palette (gold,
/// orchid, light sky blue) for a look users are likely already familiar with.
const BRACKET_PAIR_COLORS_DARK: [Color; 3] = [
    Color { r: 1.0, g: 0.843, b: 0.0, a: 1.0 }, // gold
    Color { r: 0.855, g: 0.439, b: 0.839, a: 1.0 }, // orchid
    Color { r: 0.529, g: 0.808, b: 0.980, a: 1.0 }, // light sky blue
];

/// The same cycle for a **light** editor background, following VS Code Light+.
///
/// The dark palette is built from bright, low-contrast-on-white hues; reusing
/// it on a light background leaves brackets barely visible, so the light theme
/// gets its own saturated, dark-toned triple.
const BRACKET_PAIR_COLORS_LIGHT: [Color; 3] = [
    Color { r: 0.016, g: 0.192, b: 0.980, a: 1.0 }, // blue
    Color { r: 0.192, g: 0.576, b: 0.192, a: 1.0 }, // green
    Color { r: 0.482, g: 0.220, b: 0.078, a: 1.0 }, // brown
];

impl CodeEditor {
    /// Returns the rainbow-bracket palette matching this editor's theme.
    ///
    /// Like the syntect token palette (see [`CodeEditor::resolve_syntax`]),
    /// the cycle is picked from the lightness of the style's background, so
    /// brackets stay legible under both light and dark themes.
    ///
    /// # Returns
    ///
    /// The three-color cycle to index by nesting depth.
    pub(super) fn bracket_pair_colors(&self) -> &'static [Color; 3] {
        if crate::theme::is_dark_background(self.style.background) {
            &BRACKET_PAIR_COLORS_DARK
        } else {
            &BRACKET_PAIR_COLORS_LIGHT
        }
    }

    /// Resolves the syntect syntax and theme used to color this editor's text.
    ///
    /// Both the syntax set and the theme set are process-wide singletons, and
    /// the per-editor result is memoized in [`ResolvedSyntax`], so this is
    /// cheap enough to call on every render -- the underlying
    /// `find_syntax_by_extension` scan is not. Common language aliases and
    /// extensions (`python`/`py`, `markdown`/`md`, …) are normalized here, and
    /// an unknown syntax falls back to plain text rather than losing the text.
    ///
    /// The token palette follows the editor's own theme: a light editor
    /// background selects `base16-ocean.light`, a dark one
    /// `base16-ocean.dark`.
    ///
    /// # Returns
    ///
    /// The shared syntax set, the syntax to tokenize with, and the theme
    /// providing token colors. The syntax and theme are `None` only when
    /// syntect ships no definition at all for them.
    pub(crate) fn resolve_syntax(
        &self,
    ) -> (
        &'static SyntaxSet,
        Option<&'static SyntaxReference>,
        Option<&'static syntect::highlighting::Theme>,
    ) {
        let syntax_set = SYNTAX_SET.get_or_init(|| {
            #[cfg(feature = "two-face")]
            {
                two_face::syntax::extra_newlines()
            }
            #[cfg(not(feature = "two-face"))]
            {
                SyntaxSet::load_defaults_newlines()
            }
        });

        // Pair the token palette with the editor's own theme: a dark-tuned
        // palette on a light background leaves comments and strings unreadable.
        let dark_background =
            crate::theme::is_dark_background(self.style.background);

        let mut memo = self.resolved_syntax.borrow_mut();
        if let Some((syntax, theme)) = memo
            .as_ref()
            .and_then(|resolved| resolved.get(&self.syntax, dark_background))
        {
            return (syntax_set, syntax, theme);
        }

        let theme_set = THEME_SET.get_or_init(ThemeSet::load_defaults);
        let theme_name = if dark_background {
            "base16-ocean.dark"
        } else {
            "base16-ocean.light"
        };
        let theme = theme_set
            .themes
            .get(theme_name)
            .or_else(|| theme_set.themes.values().next());

        // Normalize common language aliases/extensions used by consumers.
        let syntax = match self.syntax.as_str() {
            "python" => syntax_set.find_syntax_by_extension("py"),
            "rust" => syntax_set.find_syntax_by_extension("rs"),
            "javascript" => syntax_set.find_syntax_by_extension("js"),
            "htm" => syntax_set.find_syntax_by_extension("html"),
            "svg" => syntax_set.find_syntax_by_extension("xml"),
            "markdown" => syntax_set.find_syntax_by_extension("md"),
            "text" => Some(syntax_set.find_syntax_plain_text()),
            _ => syntax_set.find_syntax_by_extension(self.syntax.as_str()),
        }
        .or(Some(syntax_set.find_syntax_plain_text()));

        *memo = Some(ResolvedSyntax::new(
            &self.syntax,
            dark_background,
            syntax,
            theme,
        ));

        (syntax_set, syntax, theme)
    }

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
    pub(crate) fn highlighted_line_cached(
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
            *guard = Some(HighlightCache::new(self.syntax.clone()));
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
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use syntect::parsing::SyntaxSet;

    use super::*;

    #[test]
    fn test_bracket_pair_colors_follow_the_editor_theme() {
        let mut editor = CodeEditor::new("(a)", "rs");

        editor.set_theme(crate::theme::from_iced_theme(&iced::Theme::Dark));
        let dark = editor.bracket_pair_colors();

        editor.set_theme(crate::theme::from_iced_theme(&iced::Theme::Light));
        let light = editor.bracket_pair_colors();

        assert_ne!(
            dark[0], light[0],
            "a light theme must not reuse the dark rainbow palette"
        );
    }

    #[test]
    fn test_bracket_pair_colors_contrast_with_their_background() {
        // Every dark-theme bracket color must read as light (drawn on a dark
        // background) and every light-theme one as dark, or the brackets
        // vanish into the page.
        for color in BRACKET_PAIR_COLORS_DARK {
            assert!(
                !crate::theme::is_dark_background(color),
                "{color:?} is too dark for a dark background"
            );
        }
        for color in BRACKET_PAIR_COLORS_LIGHT {
            assert!(
                crate::theme::is_dark_background(color),
                "{color:?} is too light for a light background"
            );
        }
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
                .map(super::HighlightCache::valid_len),
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
}
