//! Scope-based token color overrides (see [`crate::TokenColors`]).
//!
//! syntect colors a token from the theme alone. To recolor token *kinds*
//! (keys, strings, numbers, ...) independently of the theme, each highlighted
//! range is matched against its TextMate scope stack, replayed here from the
//! parser's scope operations.

use std::sync::OnceLock;

use iced::Color;
use syntect::parsing::{Scope, ScopeStack, ScopeStackOp};

use crate::theme::TokenColors;

/// Scope prefixes that select each [`TokenColors`] kind.
struct TokenScopes {
    /// Any of these anywhere in the stack marks an object key.
    key: Vec<Scope>,
    string: Scope,
    number: Scope,
    literal: Scope,
    comment: Scope,
    punctuation: Scope,
    /// Delimiters (quotes, comment markers) take the color of what they
    /// enclose rather than the punctuation color.
    delimiter: Scope,
    /// `meta` scopes only group tokens and never name one.
    meta: Scope,
}

impl TokenScopes {
    /// Returns the process-wide parsed scopes, or `None` if syntect rejects
    /// one of the names (it never does for these literals).
    fn get() -> Option<&'static Self> {
        static SCOPES: OnceLock<Option<TokenScopes>> = OnceLock::new();
        SCOPES.get_or_init(Self::parse).as_ref()
    }

    fn parse() -> Option<Self> {
        let scope = |name: &str| Scope::new(name).ok();
        Some(Self {
            key: [
                "meta.mapping.key",
                "meta.structure.dictionary.key",
                "meta.object-literal.key",
                "support.type.property-name",
                "entity.name.tag.yaml",
            ]
            .into_iter()
            .map(scope)
            .collect::<Option<Vec<_>>>()?,
            string: scope("string")?,
            number: scope("constant.numeric")?,
            literal: scope("constant.language")?,
            comment: scope("comment")?,
            punctuation: scope("punctuation")?,
            delimiter: scope("punctuation.definition")?,
            meta: scope("meta")?,
        })
    }

    /// Returns the override for a token whose scope stack (outermost first)
    /// is `stack`, or `None` to keep the theme's color.
    fn color(&self, colors: &TokenColors, stack: &[Scope]) -> Option<Color> {
        let is_key =
            |scope: &Scope| self.key.iter().any(|key| key.is_prefix_of(*scope));
        if colors.key.is_some() && stack.iter().any(is_key) {
            return colors.key;
        }
        // Comments win over what they contain (a quoted word in a comment).
        if stack.iter().any(|scope| self.comment.is_prefix_of(*scope)) {
            return colors.comment;
        }
        // Otherwise the innermost scope naming a token decides.
        let token = stack.iter().rev().find(|scope| {
            !self.meta.is_prefix_of(**scope)
                && !self.delimiter.is_prefix_of(**scope)
        })?;
        [
            (self.string, colors.string),
            (self.number, colors.number),
            (self.literal, colors.literal),
            (self.punctuation, colors.punctuation),
        ]
        .into_iter()
        .find(|(prefix, _)| prefix.is_prefix_of(*token))
        .and_then(|(_, color)| color)
    }
}

/// Replays one line's scope operations alongside the highlighter, so each
/// highlighted range can be matched against [`TokenColors`].
pub(super) struct TokenColorizer<'a> {
    colors: TokenColors,
    scopes: &'static TokenScopes,
    stack: ScopeStack,
    ops: std::iter::Peekable<std::slice::Iter<'a, (usize, ScopeStackOp)>>,
}

impl<'a> TokenColorizer<'a> {
    /// Starts a line, or returns `None` when `colors` overrides nothing, so
    /// the common case pays no scope bookkeeping.
    ///
    /// # Arguments
    ///
    /// * `colors` - The override colors
    /// * `stack` - The scope stack in effect at the start of the line
    /// * `ops` - The line's scope operations, as returned by the parser
    pub(super) fn new(
        colors: TokenColors,
        stack: ScopeStack,
        ops: &'a [(usize, ScopeStackOp)],
    ) -> Option<Self> {
        if colors.is_empty() {
            return None;
        }
        Some(Self {
            colors,
            scopes: TokenScopes::get()?,
            stack,
            ops: ops.iter().peekable(),
        })
    }

    /// Returns the override color for the range starting at byte `offset`,
    /// or `None` to keep the theme's. Offsets must not decrease between
    /// calls.
    pub(super) fn color_at(&mut self, offset: usize) -> Option<Color> {
        while let Some((_, op)) =
            self.ops.next_if(|(position, _)| *position <= offset)
        {
            let _ = self.stack.apply(op);
        }
        self.scopes.color(&self.colors, self.stack.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use iced::Color;

    use crate::canvas_editor::CodeEditor;
    use crate::theme::{TokenColors, from_iced_theme};

    const KEY: Color = Color::from_rgb(0.9, 0.1, 0.1);
    const STRING: Color = Color::from_rgb(0.1, 0.9, 0.1);
    const NUMBER: Color = Color::from_rgb(0.1, 0.1, 0.9);
    const LITERAL: Color = Color::from_rgb(0.9, 0.9, 0.1);
    const COMMENT: Color = Color::from_rgb(0.6, 0.3, 0.9);
    const PUNCTUATION: Color = Color::from_rgb(0.5, 0.5, 0.5);

    fn all_token_colors() -> TokenColors {
        TokenColors {
            key: Some(KEY),
            string: Some(STRING),
            number: Some(NUMBER),
            literal: Some(LITERAL),
            comment: Some(COMMENT),
            punctuation: Some(PUNCTUATION),
        }
    }

    /// Highlights every line of `code` with `colors` and returns the
    /// `(text, color)` spans, whitespace-only spans dropped.
    fn spans(
        code: &str,
        syntax: &str,
        colors: TokenColors,
    ) -> Vec<(String, Color)> {
        let mut editor = CodeEditor::new(code, syntax);
        let mut style = from_iced_theme(&iced::Theme::Dark);
        style.token_colors = colors;
        editor.set_theme(style);
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return Vec::new();
        };
        (0..editor.buffer.line_count())
            .flat_map(|line| {
                editor
                    .highlighted_line_cached(line, syntax, theme, syntax_set)
                    .iter()
                    .filter(|(_, text)| !text.trim().is_empty())
                    .map(|(color, text)| (text.trim().to_owned(), *color))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Colors of the spans whose trimmed text is `text`.
    fn colors_of(spans: &[(String, Color)], text: &str) -> Vec<Color> {
        spans
            .iter()
            .filter(|(span, _)| span == text)
            .map(|(_, color)| *color)
            .collect()
    }

    #[test]
    fn test_token_colors_apply_by_scope_in_json() {
        let spans = spans(
            r#"{"name": "Ada", "age": 36, "admin": true, "boss": null}"#,
            "json",
            all_token_colors(),
        );

        // Keys win over the string color they are also scoped as.
        assert!(colors_of(&spans, "name").iter().all(|c| *c == KEY));
        assert!(!colors_of(&spans, "name").is_empty());
        assert_eq!(colors_of(&spans, "Ada"), vec![STRING]);
        assert_eq!(colors_of(&spans, "36"), vec![NUMBER]);
        assert_eq!(colors_of(&spans, "true"), vec![LITERAL]);
        assert_eq!(colors_of(&spans, "null"), vec![LITERAL]);
        assert!(colors_of(&spans, "{").iter().all(|c| *c == PUNCTUATION));
    }

    #[test]
    fn test_token_colors_match_yaml_and_javascript_scopes() {
        let yaml =
            spans("title: hello\ncount: 3\n", "yaml", all_token_colors());
        assert_eq!(colors_of(&yaml, "title"), vec![KEY]);
        assert_eq!(colors_of(&yaml, "3"), vec![NUMBER]);

        let js = spans(
            "const s = \"text\"; // note\nconst n = 42;",
            "js",
            all_token_colors(),
        );
        assert!(colors_of(&js, "text").iter().all(|c| *c == STRING));
        assert!(!colors_of(&js, "text").is_empty());
        assert_eq!(colors_of(&js, "42"), vec![NUMBER]);
        // The comment, its `//` marker included, takes the comment color.
        assert!(
            js.iter()
                .filter(|(text, _)| text.contains("note"))
                .all(|(_, color)| *color == COMMENT)
        );
    }

    #[test]
    fn test_unset_kinds_keep_the_theme_colors() {
        let code = r#"{"name": "Ada", "age": 36}"#;
        let themed = spans(code, "json", TokenColors::default());
        let only_numbers = spans(
            code,
            "json",
            TokenColors { number: Some(NUMBER), ..TokenColors::default() },
        );

        assert_eq!(themed.len(), only_numbers.len());
        for ((text, theme_color), (_, color)) in
            themed.iter().zip(&only_numbers)
        {
            if text == "36" {
                assert_eq!(*color, NUMBER);
            } else {
                assert_eq!(color, theme_color, "{text:?} must keep its color");
            }
        }
    }

    #[test]
    fn test_changing_token_colors_recolors_cached_lines() {
        let mut editor = CodeEditor::new("[1, 2]", "json");
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return;
        };
        let before =
            editor.highlighted_line_cached(0, syntax, theme, syntax_set);

        let mut style = editor.style;
        style.token_colors.number = Some(NUMBER);
        editor.set_theme(style);
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        let (Some(syntax), Some(theme)) = (syntax, theme) else {
            return;
        };
        let after =
            editor.highlighted_line_cached(0, syntax, theme, syntax_set);

        assert!(!before.iter().any(|(color, _)| *color == NUMBER));
        assert!(
            after.iter().any(|(color, text)| text == "1" && *color == NUMBER)
        );
    }

    #[test]
    fn test_syntax_theme_name_selects_a_bundled_theme() {
        let code = "fn main() { let x = \"s\"; }";
        let colors_with = |name: Option<&'static str>| -> Vec<Color> {
            let mut editor = CodeEditor::new(code, "rs");
            let mut style = from_iced_theme(&iced::Theme::Dark);
            style.syntax_theme_name = name;
            editor.set_theme(style);
            let (syntax_set, syntax, theme) = editor.resolve_syntax();
            let (Some(syntax), Some(theme)) = (syntax, theme) else {
                return Vec::new();
            };
            editor
                .highlighted_line_cached(0, syntax, theme, syntax_set)
                .iter()
                .map(|(color, _)| *color)
                .collect()
        };

        let automatic = colors_with(None);
        assert_ne!(colors_with(Some("base16-mocha.dark")), automatic);
        assert_eq!(
            colors_with(Some("no such theme")),
            automatic,
            "an unknown name falls back to the automatic theme"
        );
        assert_eq!(colors_with(Some("base16-ocean.dark")), automatic);
    }
}
