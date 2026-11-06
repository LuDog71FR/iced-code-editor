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
use std::sync::{Mutex, OnceLock};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    HighlightState, Highlighter, RangedHighlightIterator, Style, ThemeSet,
};
use syntect::parsing::{
    ParseState, ScopeStack, SyntaxDefinition, SyntaxReference, SyntaxSet,
};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use super::token_colors::TokenColorizer;
use crate::canvas_editor::{
    CodeEditor, HIGHLIGHT_CLOCK_CHECK_INTERVAL, HighlightCache, ResolvedSyntax,
};

/// Loading syntect's syntax definitions is expensive, so the set is built once
/// per process and shared by every editor instance.
static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();

/// Definitions added with [`register_syntaxes`], waiting for [`SYNTAX_SET`] to
/// be built.
static EXTRA_SYNTAXES: Mutex<Vec<SyntaxDefinition>> = Mutex::new(Vec::new());

/// Error returned by [`register_syntaxes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxRegistrationError {
    /// A definition is not a valid `.sublime-syntax` file. `index` is its
    /// position in the input; nothing was registered.
    Parse {
        /// Position of the definition in the input.
        index: usize,
        /// The parser's message.
        message: String,
    },
    /// An editor has already highlighted text, and the syntax set is built
    /// once per process: register syntaxes before creating editors.
    AlreadyBuilt,
}

impl std::fmt::Display for SyntaxRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse { index, message } => {
                write!(f, "syntax definition {index}: {message}")
            }
            Self::AlreadyBuilt => write!(
                f,
                "the syntax set is already built; register syntaxes before \
                 the first editor highlights text"
            ),
        }
    }
}

impl std::error::Error for SyntaxRegistrationError {}

/// Adds syntax definitions, in Sublime Text's `.sublime-syntax` format, to the
/// set every editor highlights with.
///
/// A definition is chosen like the bundled ones, by the file extensions it
/// lists (`file_extensions`) matching the editor's syntax name. When several
/// definitions claim the same extension, the one registered last wins, so a
/// registered syntax can replace a bundled one. Definitions can include each
/// other and the bundled ones by scope (`include: scope:source.rust`).
///
/// The set is built once per process, the first time an editor highlights
/// text, so call this before that, typically at startup. All or nothing: if
/// one definition does not parse, none is registered.
///
/// # Errors
///
/// [`SyntaxRegistrationError::Parse`] for an invalid definition, and
/// [`SyntaxRegistrationError::AlreadyBuilt`] when the set already exists.
///
/// # Example
///
/// ```
/// let ini = r#"
/// name: Tiny INI
/// file_extensions: [tini]
/// scope: source.tini
/// contexts:
///   main:
///     - match: '^\[.*\]$'
///       scope: entity.name.section.tini
/// "#;
/// iced_code_editor::register_syntaxes([ini]).expect("valid syntax");
/// ```
pub fn register_syntaxes<'a>(
    definitions: impl IntoIterator<Item = &'a str>,
) -> Result<(), SyntaxRegistrationError> {
    register_into(&SYNTAX_SET, &EXTRA_SYNTAXES, definitions)
}

/// [`register_syntaxes`] against an explicit set and queue (tests use their
/// own, so they don't depend on the process-wide set).
fn register_into<'a>(
    set: &OnceLock<SyntaxSet>,
    queue: &Mutex<Vec<SyntaxDefinition>>,
    definitions: impl IntoIterator<Item = &'a str>,
) -> Result<(), SyntaxRegistrationError> {
    let parsed = definitions
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            SyntaxDefinition::load_from_str(text, true, None).map_err(|e| {
                SyntaxRegistrationError::Parse { index, message: e.to_string() }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Hold the queue while checking, so the set can't be built in between.
    let mut queue = queue.lock().unwrap_or_else(|e| e.into_inner());
    if set.get().is_some() {
        return Err(SyntaxRegistrationError::AlreadyBuilt);
    }
    queue.extend(parsed);
    Ok(())
}

/// The bundled syntaxes plus the registered ones (see [`register_syntaxes`]).
fn build_syntax_set(queue: &Mutex<Vec<SyntaxDefinition>>) -> SyntaxSet {
    #[cfg(feature = "two-face")]
    let base = two_face::syntax::extra_newlines();
    #[cfg(not(feature = "two-face"))]
    let base = SyntaxSet::load_defaults_newlines();

    let extra =
        std::mem::take(&mut *queue.lock().unwrap_or_else(|e| e.into_inner()));
    if extra.is_empty() {
        return base;
    }
    let mut builder = base.into_builder();
    for definition in extra {
        builder.add(definition);
    }
    builder.build()
}

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
    color_from_syntect(style.foreground)
}

/// Converts a syntect [`syntect::highlighting::Color`] into an opaque iced
/// [`Color`].
fn color_from_syntect(color: syntect::highlighting::Color) -> Color {
    Color::from_rgb(
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
    )
}

/// Perceptual luminance (ITU-R BT.601), used only to compare contrast.
fn luminance(color: Color) -> f32 {
    color.r * 0.299 + color.g * 0.587 + color.b * 0.114
}

/// Minimum luminance separation for a token to count as legible.
const MIN_CONTRAST: f32 = 0.12;

/// Whether `color` stands out enough from `background` to be read.
fn contrasts_with(color: Color, background: Color) -> bool {
    (luminance(color) - luminance(background)).abs() >= MIN_CONTRAST
}

/// Picks a foreground color for `style` that is legible on `background`.
///
/// Some syntect themes express a scope as a *background* highlight: the
/// base16 themes paint `invalid.illegal` as "foreground = the theme
/// background, background = red". Drawing only the foreground on the editor
/// background therefore made half-typed code (an unmatched brace, a stray
/// comma) disappear until it became valid again.
///
/// Resolution order: the style's foreground if it contrasts, then its
/// background (the color the theme chose to stand out, so invalid tokens
/// read as red), then `fallback`, the editor's plain text color.
fn readable_color(style: Style, background: Color, fallback: Color) -> Color {
    let foreground = color_from_style(style);
    if contrasts_with(foreground, background) {
        return foreground;
    }
    let highlight = color_from_syntect(style.background);
    if contrasts_with(highlight, background) {
        return highlight;
    }
    fallback
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

    // Check contrast against the theme's own colors, for the same reason as
    // the rendering path (see `readable_color`).
    let background =
        theme.settings.background.map_or(Color::BLACK, color_from_syntect);
    let fallback =
        theme.settings.foreground.map_or(Color::WHITE, color_from_syntect);

    ranges
        .into_iter()
        .map(|(style, text)| {
            (readable_color(style, background, fallback), text.to_string())
        })
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
    /// The token palette is [`crate::Style::syntax_theme_name`] when set to a
    /// bundled theme; otherwise it follows the editor's own theme: a light
    /// editor background selects `base16-ocean.light`, a dark one
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
        let syntax_set =
            SYNTAX_SET.get_or_init(|| build_syntax_set(&EXTRA_SYNTAXES));

        // Pair the token palette with the editor's own theme: a dark-tuned
        // palette on a light background leaves comments and strings unreadable.
        let dark_background =
            crate::theme::is_dark_background(self.style.background);

        let requested_theme = self.style.syntax_theme_name;

        let mut memo = self.resolved_syntax.borrow_mut();
        if let Some((syntax, theme)) = memo.as_ref().and_then(|resolved| {
            resolved.get(&self.syntax, dark_background, requested_theme)
        }) {
            return (syntax_set, syntax, theme);
        }

        let theme_set = THEME_SET.get_or_init(ThemeSet::load_defaults);
        let automatic_theme = if dark_background {
            "base16-ocean.dark"
        } else {
            "base16-ocean.light"
        };
        // A host-chosen theme wins; an unknown name falls back to the
        // automatic one rather than losing the colors.
        let theme = requested_theme
            .and_then(|name| theme_set.themes.get(name))
            .or_else(|| theme_set.themes.get(automatic_theme))
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
            requested_theme,
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
        let background = self.style.background;
        let fallback = self.style.text_color;
        let deadline = self.highlight_deadline.get();
        let mut parsed = 0usize;
        if lines_to_parse > 0 {
            for index in cache.valid_len()..=parse_end {
                // Stop when this frame's time budget runs out, even if the
                // line budget has not. The clock is only read every few lines.
                if parsed > 0
                    && parsed.is_multiple_of(HIGHLIGHT_CLOCK_CHECK_INTERVAL)
                    && deadline
                        .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    break;
                }
                parsed += 1;
                // syntect's `_newlines` syntaxes expect a trailing '\n' for correct
                // end-of-line context handling; the stored buffer line has none.
                let mut line = self.buffer.line(index).to_string();
                line.push('\n');

                let ops = parse_state
                    .parse_line(&line, syntax_set)
                    .unwrap_or_default();
                // Captured before the highlighter advances the scope stack.
                let mut token_colors = TokenColorizer::new(
                    self.style.token_colors,
                    highlight_state.path.clone(),
                    &ops,
                );
                let spans: Vec<(Color, String)> = RangedHighlightIterator::new(
                    &mut highlight_state,
                    &ops,
                    &line,
                    &highlighter,
                )
                .filter_map(|(style, text, range)| {
                    let text = text.strip_suffix('\n').unwrap_or(text);
                    if text.is_empty() {
                        return None;
                    }
                    let color = token_colors
                        .as_mut()
                        .and_then(|colors| colors.color_at(range.start))
                        .unwrap_or_else(|| {
                            readable_color(style, background, fallback)
                        });
                    Some((color, text.to_string()))
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
        // Charge only the lines actually parsed: stopping on the time budget
        // must not consume the rest of the line budget.
        self.highlight_lines_remaining
            .set(self.highlight_lines_remaining.get().saturating_sub(parsed));
        // The target is still out of reach: ask for another frame. The line
        // budget is only refilled when the content layer is redrawn, so
        // without this the catch-up would stop until an unrelated redraw.
        if cache.valid_len() <= target {
            self.highlight_catch_up_pending.set(true);
        }

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
    fn test_expired_time_budget_stops_parsing_early() {
        let content = vec!["{\"k\": 1}"; 1_000].join("\n");
        let editor = CodeEditor::new(&content, "json");
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return;
        };
        // Plenty of line budget, but the frame's clock already ran out.
        editor.highlight_lines_remaining.set(10_000);
        editor.highlight_deadline.set(Some(Instant::now()));

        let _ = editor.highlighted_line_cached(999, syntax, theme, syntax_set);

        let parsed = editor
            .highlight_cache
            .borrow()
            .as_ref()
            .map(super::HighlightCache::valid_len);
        assert_eq!(
            parsed,
            Some(HIGHLIGHT_CLOCK_CHECK_INTERVAL),
            "parsing stops at the first clock check past the deadline"
        );
        assert_eq!(
            editor.highlight_lines_remaining.get(),
            10_000 - HIGHLIGHT_CLOCK_CHECK_INTERVAL,
            "only the lines actually parsed are charged"
        );
        assert!(editor.highlight_catch_up_pending.get());
    }

    #[test]
    fn test_no_deadline_parses_up_to_the_target() {
        let content = vec!["{\"k\": 1}"; 1_000].join("\n");
        let editor = CodeEditor::new(&content, "json");
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return;
        };

        let _ = editor.highlighted_line_cached(999, syntax, theme, syntax_set);

        assert!(!editor.highlight_catch_up_pending.get());
        assert_eq!(
            editor
                .highlight_cache
                .borrow()
                .as_ref()
                .map(super::HighlightCache::valid_len),
            Some(1_000)
        );
    }

    /// Perceptual luminance (ITU-R BT.601), recomputed locally so the test
    /// does not depend on the code under test.
    fn test_luminance(color: Color) -> f32 {
        color.r * 0.299 + color.g * 0.587 + color.b * 0.114
    }

    /// Every rendered span of `code` must stay legible on the editor
    /// background, for both the light and the dark palette.
    fn assert_all_spans_legible(code: &str, syntax_name: &str) {
        for iced_theme in [iced::Theme::Dark, iced::Theme::Light] {
            let mut editor = CodeEditor::new(code, syntax_name);
            editor.set_theme(crate::theme::from_iced_theme(&iced_theme));
            let (syntax_set, syntax, theme) = editor.resolve_syntax();
            let (Some(syntax), Some(theme)) = (syntax, theme) else {
                continue;
            };
            let background = test_luminance(editor.style.background);

            for line in 0..editor.buffer.line_count() {
                let spans = editor
                    .highlighted_line_cached(line, syntax, theme, syntax_set);
                for (color, text) in spans.iter() {
                    let contrast = (test_luminance(*color) - background).abs();
                    assert!(
                        contrast >= 0.12,
                        "{iced_theme:?}: {text:?} on line {line} is drawn in \
                         {color:?}, which vanishes into the background"
                    );
                }
            }
        }
    }

    #[test]
    fn test_invalid_json_tokens_stay_legible() {
        // A half-typed JSON body: the grammar flags the stray tokens as
        // `invalid.illegal`, which base16 themes paint as background-coloured
        // text on a red block.
        assert_all_spans_legible("{\n  \"a\": 1,,\n  \"b\" 2\n}}\n]", "json");
    }

    #[test]
    fn test_valid_code_keeps_distinct_theme_colors() {
        // The legibility fallback must not flatten ordinary highlighting.
        let editor = CodeEditor::new("{\"key\": \"value\", \"n\": 42}", "json");
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return;
        };
        let spans =
            editor.highlighted_line_cached(0, syntax, theme, syntax_set);
        let mut colors: Vec<Color> = Vec::new();
        for (color, _) in spans.iter() {
            if !colors.contains(color) {
                colors.push(*color);
            }
        }
        assert!(colors.len() >= 3, "expected varied token colors: {colors:?}");
        assert_all_spans_legible("{\"key\": \"value\", \"n\": 42}", "json");
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

    const TINY: &str = r#"
name: Tiny
file_extensions: [tiny]
scope: source.tiny
contexts:
  main:
    - match: '\bhello\b'
      scope: keyword.tiny
"#;

    #[test]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    fn registered_syntaxes_join_the_bundled_ones() {
        let set = OnceLock::new();
        let queue = Mutex::new(Vec::new());
        register_into(&set, &queue, [TINY]).expect("valid");
        let built = set.get_or_init(|| build_syntax_set(&queue));

        let tiny = built.find_syntax_by_extension("tiny").expect("registered");
        assert_eq!(tiny.name, "Tiny");
        // The bundled ones are still there.
        assert!(built.find_syntax_by_extension("rs").is_some());
        assert!(queue.lock().unwrap().is_empty(), "the queue is consumed");

        let spans = highlight_line_spans(
            "say hello",
            tiny,
            &syntect::highlighting::Theme::default(),
            built,
        );
        let texts: Vec<_> = spans.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(texts.concat(), "say hello");
    }

    #[test]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    fn a_registered_syntax_can_include_a_bundled_one_and_replace_it() {
        let wrapper = r#"
name: Rusty
file_extensions: [rs]
scope: source.rusty
contexts:
  main:
    - include: scope:source.rust
"#;
        let set = OnceLock::new();
        let queue = Mutex::new(Vec::new());
        register_into(&set, &queue, [wrapper]).expect("valid");
        let built = set.get_or_init(|| build_syntax_set(&queue));
        let rs = built.find_syntax_by_extension("rs").expect("rs");
        assert_eq!(rs.name, "Rusty", "registered last wins");
        // Parsing through the include does not panic.
        let mut state = ParseState::new(rs);
        state.parse_line("fn main() {}\n", built).expect("parses");
    }

    #[test]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    fn invalid_definitions_and_late_registration_are_rejected() {
        let set = OnceLock::new();
        let queue = Mutex::new(Vec::new());
        let error = register_into(&set, &queue, [TINY, "name: [unclosed"])
            .expect_err("invalid");
        assert!(matches!(
            error,
            SyntaxRegistrationError::Parse { index: 1, .. }
        ));
        assert!(queue.lock().unwrap().is_empty(), "all or nothing");

        set.get_or_init(|| build_syntax_set(&queue));
        assert_eq!(
            register_into(&set, &queue, [TINY]),
            Err(SyntaxRegistrationError::AlreadyBuilt)
        );
    }

    #[test]
    #[allow(clippy::unwrap_used, clippy::expect_used)]
    fn an_include_of_a_missing_syntax_does_not_panic() {
        let dangling = r#"
name: Dangling
file_extensions: [dangling]
scope: source.dangling
contexts:
  main:
    - include: scope:source.does-not-exist
    - match: '\bok\b'
      scope: keyword.dangling
"#;
        let set = OnceLock::new();
        let queue = Mutex::new(Vec::new());
        register_into(&set, &queue, [dangling]).expect("valid");
        let built = set.get_or_init(|| build_syntax_set(&queue));
        let syntax = built.find_syntax_by_extension("dangling").expect("registered");
        let mut state = ParseState::new(syntax);
        let _ = state.parse_line("ok then\n", built);
    }
}
