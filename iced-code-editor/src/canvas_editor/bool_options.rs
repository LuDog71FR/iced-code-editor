//! The mechanical half of the boolean editor options.
//!
//! Each option is up to three methods — `set_x`, `x`, `with_x` — and only the
//! setter carries information: what the feature is, and which cache toggling it
//! has to invalidate. Those stay hand-written in [`super::config`]. The getter
//! and the builder are pure boilerplate, about fifty lines apiece once the
//! mandatory `# Returns` / `# Arguments` / `# Example` sections are written
//! out, and are generated here from a table instead.
//!
//! Two things this buys beyond the line count. The builder bodies were split
//! between assigning the field directly and delegating to the setter, and the
//! difference was accidental: a builder that assigns silently skips whatever
//! the setter does besides assigning, which is a trap for the next option that
//! grows a side effect. Every generated builder delegates. And the doc shape is
//! now stated once, so an option cannot arrive with a section missing.
//!
//! Each row also carries the option's default, which the generated example
//! *asserts* rather than merely stating in a comment the way the hand-written
//! getters did — so a default that changes fails a doctest instead of quietly
//! contradicting its own documentation.

/// Generates the getter and the optional builder for a boolean editor option.
///
/// Each row names the field (which is also the getter's name), its setter, and
/// optionally its builder, followed by the wording rustdoc needs. See the
/// module documentation for why only these two are generated.
macro_rules! bool_options {
    ($(
        $get:ident, $set:ident $(, $with:ident)?,
            $get_summary:literal,
            $get_returns:literal,
            default: $default_note:literal, $default_negation:literal
            $(builder: $with_summary:literal,
              $with_argument:literal)?;
    )*) => {
        impl CodeEditor {
            $(
                #[doc = $get_summary]
                #[doc = ""]
                #[doc = "# Returns"]
                #[doc = ""]
                #[doc = $get_returns]
                #[doc = ""]
                #[doc = "# Example"]
                #[doc = ""]
                #[doc = "```"]
                #[doc = "use iced_code_editor::CodeEditor;"]
                #[doc = ""]
                #[doc = "let mut editor = CodeEditor::new(\"fn main() {}\", \"rs\");"]
                #[doc = concat!("// ", $default_note)]
                #[doc = concat!(
                    "assert!(",
                    $default_negation,
                    "editor.",
                    stringify!($get),
                    "());"
                )]
                #[doc = ""]
                #[doc = concat!("editor.", stringify!($set), "(true);")]
                #[doc = concat!("assert!(editor.", stringify!($get), "());")]
                #[doc = ""]
                #[doc = concat!("editor.", stringify!($set), "(false);")]
                #[doc = concat!("assert!(!editor.", stringify!($get), "());")]
                #[doc = "```"]
                #[must_use]
                pub fn $get(&self) -> bool {
                    self.$get
                }

                $(
                    #[doc = $with_summary]
                    #[doc = ""]
                    #[doc = "# Arguments"]
                    #[doc = ""]
                    #[doc = concat!("* `enabled` - ", $with_argument)]
                    #[doc = ""]
                    #[doc = "# Returns"]
                    #[doc = ""]
                    #[doc = "Self for method chaining"]
                    #[doc = ""]
                    #[doc = "# Example"]
                    #[doc = ""]
                    #[doc = "```"]
                    #[doc = "use iced_code_editor::CodeEditor;"]
                    #[doc = ""]
                    #[doc = concat!(
                        "let editor = CodeEditor::new(\"fn main() {}\", \"rs\")"
                    )]
                    #[doc = concat!("    .", stringify!($with), "(false);")]
                    #[doc = concat!("assert!(!editor.", stringify!($get), "());")]
                    #[doc = "```"]
                    #[must_use]
                    pub fn $with(mut self, enabled: bool) -> Self {
                        // Through the setter, never a bare assignment: the
                        // builder must not be the path that forgets a side
                        // effect the setter has.
                        self.$set(enabled);
                        self
                    }
                )?
            )*
        }
    };
}

use super::CodeEditor;

bool_options! {
    auto_close_brackets, set_auto_close_brackets,
        "Returns whether auto-closing of brackets and quotes is enabled.",
        "`true` if auto-closing is enabled, `false` otherwise",
        default: "Enabled by default.", "";
    auto_indent_enabled, set_auto_indent_enabled,
        "Returns whether auto-indentation is enabled.",
        "`true` if auto-indentation is enabled, `false` otherwise",
        default: "Enabled by default.", "";
    bracket_match_highlight_enabled, set_bracket_match_highlight_enabled,
        "Returns whether the matching-bracket/quote-pair highlight overlay is enabled.",
        "`true` if the highlight is enabled, `false` otherwise",
        default: "Enabled by default.", "";
    bracket_pair_colorization_enabled, set_bracket_pair_colorization_enabled,
        "Returns whether bracket-pair colorization (rainbow brackets) is enabled.",
        "`true` if bracket-pair colorization is enabled, `false` otherwise",
        default: "Enabled by default.", "";
    command_palette_enabled, set_command_palette_enabled, with_command_palette_enabled,
        "Returns whether the command palette can be opened.",
        "`true` if the palette can be opened, `false` otherwise",
        default: "Available by default.", ""
        builder: "Sets command-palette availability using the builder pattern.",
        "Whether the palette can be opened";
    default_command_palette_enabled, set_default_command_palette_enabled,
        "Returns whether the built-in commands are listed in the palette.",
        "`true` if the built-in commands are listed, `false` otherwise",
        default: "Listed by default.", "";
    default_context_menu_enabled, set_default_context_menu_enabled, with_default_context_menu_enabled,
        "Returns whether the built-in context-menu actions are enabled.",
        "`true` if the built-in actions are shown, `false` otherwise",
        default: "Shown by default.", ""
        builder: "Sets built-in context-menu visibility using the builder pattern.",
        "Whether to show the built-in actions";
    folding_enabled, set_folding_enabled, with_folding_enabled,
        "Returns whether code folding is enabled.",
        "`true` if code folding is enabled, `false` otherwise",
        default: "Enabled by default.", ""
        builder: "Enables or disables code folding using the builder pattern.",
        "Whether to enable code folding";
    line_numbers_enabled, set_line_numbers_enabled, with_line_numbers_enabled,
        "Returns whether line numbers are displayed.",
        "`true` if line numbers are displayed, `false` otherwise",
        default: "Displayed by default.", ""
        builder: "Sets the line numbers display with builder pattern.",
        "Whether to display line numbers";
    reveal_in_file_manager_enabled, set_reveal_in_file_manager_enabled,
        "Returns whether the reveal-in-file-manager action is shown.",
        "`true` if the reveal action is shown, `false` otherwise",
        default: "Off by default: the editor doesn't know its own file path.", "!";
    search_replace_enabled, set_search_replace_enabled,
        "Returns whether search/replace functionality is enabled.",
        "`true` if search/replace is enabled, `false` otherwise",
        default: "Enabled by default.", "";
    show_color_previews, set_show_color_previews, with_show_color_previews,
        "Returns whether inline color previews are drawn.",
        "`true` if color-preview swatches are rendered, `false` otherwise",
        default: "Enabled by default.", ""
        builder: "Sets the inline color-preview display with builder pattern.",
        "Whether to draw color-preview swatches";
    show_indent_guides, set_show_indent_guides, with_show_indent_guides,
        "Returns whether indentation guides are drawn.",
        "`true` if indentation guides are rendered, `false` otherwise",
        default: "Enabled by default.", ""
        builder: "Sets the indentation-guide display with builder pattern.",
        "Whether to draw indentation guides";
    show_whitespace, set_show_whitespace,
        "Returns whether visible whitespace rendering is enabled.",
        "`true` if whitespace characters are rendered, `false` otherwise",
        default: "Enabled by default.", "";
    sticky_scroll_enabled, set_sticky_scroll_enabled, with_sticky_scroll_enabled,
        "Returns whether sticky scroll is enabled.",
        "`true` if enclosing block headers are pinned, `false` otherwise",
        default: "Enabled by default.", ""
        builder: "Sets sticky scroll with builder pattern.",
        "Whether to pin enclosing block headers above the viewport";
    vim_enabled, set_vim_enabled, with_vim_enabled,
        "Returns whether Vim behavior is enabled for this editor instance.",
        "`true` if Vim behavior is enabled, `false` otherwise",
        default: "Off by default.", "!"
        builder: "Sets whether Vim behavior is enabled using the builder pattern.",
        "Whether to enable Vim behavior";
    wrap_enabled, set_wrap_enabled, with_wrap_enabled,
        "Returns whether line wrapping is enabled.",
        "`true` if line wrapping is enabled, `false` otherwise",
        default: "Enabled by default.", ""
        builder: "Sets the line wrapping with builder pattern.",
        "Whether to enable line wrapping";
}
