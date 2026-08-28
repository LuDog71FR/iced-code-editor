//! Small shared value types used across the demo app.
//!
//! Includes editor/tab identifiers and `Display`-wrapping newtypes needed
//! to populate `pick_list` widgets with fonts, languages, and templates.

use iced::Font;
use iced_code_editor::{CodeEditor, Language};

/// Identifier for which editor is being referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorId(pub usize);

impl std::fmt::Display for EditorId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Editor {}", self.0)
    }
}

/// Wrapper for Font to implement Display trait for pick_list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontOption {
    pub name: &'static str,
    pub font: Font,
}

impl FontOption {
    pub const MONOSPACE: FontOption =
        FontOption { name: "Monospace (Default)", font: Font::MONOSPACE };

    pub const SERIF: FontOption = FontOption {
        name: "Serif",
        font: Font { family: iced::font::Family::Serif, ..Font::DEFAULT },
    };

    pub const SANS_SERIF: FontOption = FontOption {
        name: "Sans Serif",
        font: Font { family: iced::font::Family::SansSerif, ..Font::DEFAULT },
    };

    pub const JETBRAINS_MONO: FontOption = FontOption {
        name: "JetBrains Mono",
        font: Font {
            family: iced::font::Family::Name("JetBrains Mono"),
            ..Font::DEFAULT
        },
    };

    pub const NOTO_SANS_CJK_SC: FontOption = FontOption {
        name: "Noto Sans CJK SC",
        font: Font {
            family: iced::font::Family::Name("Noto Sans CJK SC"),
            ..Font::DEFAULT
        },
    };

    pub const ALL: [FontOption; 5] = [
        FontOption::MONOSPACE,
        FontOption::SERIF,
        FontOption::SANS_SERIF,
        FontOption::JETBRAINS_MONO,
        FontOption::NOTO_SANS_CJK_SC,
    ];

    pub fn font(&self) -> Font {
        self.font
    }
}

impl std::fmt::Display for FontOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Wrapper for Language to implement Display trait for pick_list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageOption(Language);

impl LanguageOption {
    pub const ALL: [LanguageOption; 8] = [
        LanguageOption(Language::German),
        LanguageOption(Language::English),
        LanguageOption(Language::Spanish),
        LanguageOption(Language::French),
        LanguageOption(Language::Italian),
        LanguageOption(Language::PortugueseBR),
        LanguageOption(Language::PortuguesePT),
        LanguageOption(Language::ChineseSimplified),
    ];

    pub fn inner(&self) -> Language {
        self.0
    }
}

impl From<Language> for LanguageOption {
    fn from(lang: Language) -> Self {
        LanguageOption(lang)
    }
}

impl std::fmt::Display for LanguageOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Language::English => write!(f, "English"),
            Language::French => write!(f, "Français"),
            Language::Spanish => write!(f, "Español"),
            Language::German => write!(f, "Deutsch"),
            Language::Italian => write!(f, "Italiano"),
            Language::PortugueseBR => write!(f, "Português (BR)"),
            Language::PortuguesePT => write!(f, "Português (PT)"),
            Language::ChineseSimplified => write!(f, "简体中文"),
        }
    }
}

/// Code templates available in the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Empty,
    HelloWorld,
    Fibonacci,
    Factorial,
}

impl Template {
    pub const ALL: [Template; 4] = [
        Template::Empty,
        Template::HelloWorld,
        Template::Fibonacci,
        Template::Factorial,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Template::Empty => "Empty",
            Template::HelloWorld => "Hello World",
            Template::Fibonacci => "Fibonacci",
            Template::Factorial => "Factorial",
        }
    }

    pub fn content(&self) -> &'static str {
        match self {
            Template::Empty => "",
            Template::HelloWorld => {
                r#"-- Hello World in Lua
print("Hello, World!")
"#
            }
            Template::Fibonacci => {
                r#"-- Fibonacci sequence in Lua
function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

-- Print first 10 Fibonacci numbers
for i = 0, 10 do
    print("fib(" .. i .. ") = " .. fibonacci(i))
end
"#
            }
            Template::Factorial => {
                r#"-- Factorial function in Lua
function factorial(n)
    if n <= 1 then
        return 1
    end
    return n * factorial(n - 1)
end

-- Calculate factorials
for i = 1, 10 do
    print(i .. "! = " .. factorial(i))
end
"#
            }
        }
    }
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// A boolean editor setting exposed as a checkbox in the demo app's editor
/// options panel.
///
/// Each variant maps to exactly one `CodeEditor` getter/setter pair.
/// Collecting them here — rather than one `Message` variant, one `update`
/// handler, and one hand-written checkbox per setting — is what lets
/// [`EditorToggle::ALL`] drive both the options panel layout and the
/// `update` dispatch from a single list, instead of fourteen near-identical
/// copies of the same three pieces of code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditorToggle {
    /// Soft-wraps long lines instead of scrolling horizontally.
    Wrap,
    /// Collapses/expands code blocks.
    Folding,
    /// Auto-indents new lines to match the surrounding block.
    AutoIndent,
    /// Auto-inserts the matching closing bracket/quote.
    AutoCloseBrackets,
    /// Enables the search/replace dialog and its keyboard shortcuts.
    SearchReplace,
    /// Shows line numbers in the gutter.
    LineNumbers,
    /// Renders spaces and tabs as visible glyphs.
    ShowWhitespace,
    /// Draws vertical guides at each indentation level.
    IndentGuides,
    /// Draws a color swatch next to each color literal.
    ColorPreviews,
    /// Highlights the bracket matching the one at the cursor.
    BracketMatchHighlight,
    /// Colors nested bracket pairs by nesting depth.
    BracketPairColorization,
    /// Pins the headers of the enclosing blocks above the viewport.
    StickyScroll,
    /// Enables Vim modal editing.
    Vim,
    /// Enables the LSP client for this editor.
    Lsp,
}

impl EditorToggle {
    /// Every toggle, in the order the options panel displays them.
    pub const ALL: [EditorToggle; 14] = [
        EditorToggle::Wrap,
        EditorToggle::Folding,
        EditorToggle::AutoIndent,
        EditorToggle::AutoCloseBrackets,
        EditorToggle::SearchReplace,
        EditorToggle::LineNumbers,
        EditorToggle::ShowWhitespace,
        EditorToggle::IndentGuides,
        EditorToggle::ColorPreviews,
        EditorToggle::BracketMatchHighlight,
        EditorToggle::BracketPairColorization,
        EditorToggle::StickyScroll,
        EditorToggle::Vim,
        EditorToggle::Lsp,
    ];

    /// Checkbox label and log-message name for this setting.
    pub fn label(self) -> &'static str {
        match self {
            EditorToggle::Wrap => "Line wrapping",
            EditorToggle::Folding => "Code folding",
            EditorToggle::AutoIndent => "Auto-indentation",
            EditorToggle::AutoCloseBrackets => "Auto-close brackets",
            EditorToggle::SearchReplace => "Allow search/replace",
            EditorToggle::LineNumbers => "Show line numbers",
            EditorToggle::ShowWhitespace => "Show whitespace",
            EditorToggle::IndentGuides => "Show indentation guides",
            EditorToggle::ColorPreviews => "Show color previews",
            EditorToggle::BracketMatchHighlight => "Highlight matching bracket",
            EditorToggle::BracketPairColorization => "Rainbow brackets",
            EditorToggle::StickyScroll => "Sticky scroll",
            EditorToggle::Vim => "Vim mode (Cmd/Ctrl+Alt+V)",
            EditorToggle::Lsp => "LSP",
        }
    }

    /// Reads this setting's current value from `editor`.
    pub fn is_enabled(self, editor: &CodeEditor) -> bool {
        match self {
            EditorToggle::Wrap => editor.wrap_enabled(),
            EditorToggle::Folding => editor.folding_enabled(),
            EditorToggle::AutoIndent => editor.auto_indent_enabled(),
            EditorToggle::AutoCloseBrackets => editor.auto_close_brackets(),
            EditorToggle::SearchReplace => editor.search_replace_enabled(),
            EditorToggle::LineNumbers => editor.line_numbers_enabled(),
            EditorToggle::ShowWhitespace => editor.show_whitespace(),
            EditorToggle::IndentGuides => editor.show_indent_guides(),
            EditorToggle::ColorPreviews => editor.show_color_previews(),
            EditorToggle::BracketMatchHighlight => {
                editor.bracket_match_highlight_enabled()
            }
            EditorToggle::BracketPairColorization => {
                editor.bracket_pair_colorization_enabled()
            }
            EditorToggle::StickyScroll => editor.sticky_scroll_enabled(),
            EditorToggle::Vim => editor.vim_enabled(),
            EditorToggle::Lsp => editor.lsp_enabled(),
        }
    }

    /// Applies `enabled` to this setting on `editor`.
    ///
    /// For [`EditorToggle::Lsp`] this only flips the editor's own
    /// `lsp_enabled` flag; attaching or detaching the actual language-server
    /// process is a separate, platform-gated step the caller performs
    /// alongside this call (see `DemoApp::handle_toggle_editor`).
    pub fn apply(self, editor: &mut CodeEditor, enabled: bool) {
        match self {
            EditorToggle::Wrap => editor.set_wrap_enabled(enabled),
            EditorToggle::Folding => editor.set_folding_enabled(enabled),
            EditorToggle::AutoIndent => editor.set_auto_indent_enabled(enabled),
            EditorToggle::AutoCloseBrackets => {
                editor.set_auto_close_brackets(enabled)
            }
            EditorToggle::SearchReplace => {
                editor.set_search_replace_enabled(enabled)
            }
            EditorToggle::LineNumbers => {
                editor.set_line_numbers_enabled(enabled)
            }
            EditorToggle::ShowWhitespace => editor.set_show_whitespace(enabled),
            EditorToggle::IndentGuides => {
                editor.set_show_indent_guides(enabled)
            }
            EditorToggle::ColorPreviews => {
                editor.set_show_color_previews(enabled)
            }
            EditorToggle::BracketMatchHighlight => {
                editor.set_bracket_match_highlight_enabled(enabled)
            }
            EditorToggle::BracketPairColorization => {
                editor.set_bracket_pair_colorization_enabled(enabled)
            }
            EditorToggle::StickyScroll => {
                editor.set_sticky_scroll_enabled(enabled)
            }
            EditorToggle::Vim => editor.set_vim_enabled(enabled),
            EditorToggle::Lsp => editor.set_lsp_enabled(enabled),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_editor_toggle_all_has_no_duplicates() {
        let seen: HashSet<EditorToggle> =
            EditorToggle::ALL.into_iter().collect();
        assert_eq!(seen.len(), EditorToggle::ALL.len());
    }

    #[test]
    fn test_editor_toggle_labels_are_non_empty_and_unique() {
        let mut seen = HashSet::new();
        for toggle in EditorToggle::ALL {
            let label = toggle.label();
            assert!(!label.is_empty(), "{toggle:?} has an empty label");
            assert!(seen.insert(label), "duplicate label: {label}");
        }
    }

    #[test]
    fn test_editor_toggle_apply_updates_is_enabled() {
        let mut editor = CodeEditor::new("", "txt");
        for toggle in EditorToggle::ALL {
            let before = toggle.is_enabled(&editor);
            toggle.apply(&mut editor, !before);
            assert_eq!(
                toggle.is_enabled(&editor),
                !before,
                "{toggle:?} did not update after apply()"
            );
            // Restore before moving to the next toggle so each starts from
            // the editor's default state for its own setting.
            toggle.apply(&mut editor, before);
        }
    }
}
