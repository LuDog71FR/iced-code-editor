//! Builder-style configuration: context menu, theme/language, Vim toggle,
//! wrap/whitespace/bracket/folding-enabled flags, auto-indent, auto-close
//! brackets, indent style, search/replace enablement, and line numbers.

use crate::canvas_editor::features::context_menu::{
    ContextMenuEntry, ContextMenuItem,
};
use crate::canvas_editor::features::vim::VimMode;
use crate::canvas_editor::{CodeEditor, IndentStyle};
use crate::theme::Style;

impl CodeEditor {
    /// Replaces the custom context-menu entries.
    ///
    /// Custom entries are shown in addition to the built-in editing actions;
    /// see [`Self::set_default_context_menu_enabled`] to hide those.
    ///
    /// # Arguments
    ///
    /// * `entries` - The entries to display, in order
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, ContextMenuEntry};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_custom_context_menu_entries(vec![
    ///     ContextMenuEntry::item("format", "Format Document")
    ///         .with_shortcut("Ctrl+Shift+F"),
    ///     ContextMenuEntry::separator(),
    ///     ContextMenuEntry::item("rename", "Rename Symbol").with_enabled(false),
    /// ]);
    /// assert_eq!(editor.custom_context_menu_entries().len(), 3);
    /// ```
    pub fn set_custom_context_menu_entries(
        &mut self,
        entries: Vec<ContextMenuEntry>,
    ) {
        self.custom_context_menu_entries = entries;
    }

    /// Replaces the custom context-menu entries using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `entries` - The entries to display, in order
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, ContextMenuEntry};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_custom_context_menu_entries(vec![
    ///         ContextMenuEntry::item("format", "Format Document"),
    ///     ]);
    /// assert_eq!(editor.custom_context_menu_entries().len(), 1);
    /// ```
    #[must_use]
    pub fn with_custom_context_menu_entries(
        mut self,
        entries: Vec<ContextMenuEntry>,
    ) -> Self {
        self.set_custom_context_menu_entries(entries);
        self
    }

    /// Returns the custom context-menu entries in display order.
    ///
    /// # Returns
    ///
    /// The entries previously set, or an empty slice if none were set
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!(editor.custom_context_menu_entries().is_empty());
    /// ```
    pub fn custom_context_menu_entries(&self) -> &[ContextMenuEntry] {
        &self.custom_context_menu_entries
    }

    /// Sets whether the built-in editing actions appear in the context menu.
    ///
    /// The built-in actions are undo/redo, cut/copy/paste, and select all.
    /// Disabling them leaves only the custom entries, which is how a host
    /// application replaces the menu wholesale rather than extending it.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to show the built-in actions
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_default_context_menu_enabled(false);
    /// assert!(!editor.default_context_menu_enabled());
    /// ```
    pub fn set_default_context_menu_enabled(&mut self, enabled: bool) {
        self.default_context_menu_enabled = enabled;
    }

    /// Replaces the custom command-palette entries.
    ///
    /// Custom commands are listed before the built-in editor commands; see
    /// [`Self::set_default_command_palette_enabled`] to hide those. Running
    /// one emits [`Message::CommandPaletteAction`] carrying the entry's `id`,
    /// which the host application handles — the editor never acts on it.
    ///
    /// Entries created with `with_enabled(false)` are left out of the list
    /// entirely rather than dimmed: the palette is a search result list, so
    /// every row it offers should be runnable.
    ///
    /// # Arguments
    ///
    /// * `entries` - The commands to list, in order
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, ContextMenuItem};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_custom_command_palette_entries(vec![
    ///     ContextMenuItem::new("app.open_file", "Open File")
    ///         .with_shortcut("Ctrl+O"),
    ///     ContextMenuItem::new("app.new_tab", "New Tab"),
    /// ]);
    /// assert_eq!(editor.custom_command_palette_entries().len(), 2);
    /// ```
    ///
    /// [`Message::CommandPaletteAction`]: crate::Message::CommandPaletteAction
    pub fn set_custom_command_palette_entries(
        &mut self,
        entries: Vec<ContextMenuItem>,
    ) {
        self.custom_command_palette_entries = entries;
    }

    /// Replaces the custom command-palette entries using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `entries` - The commands to list, in order
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, ContextMenuItem};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_custom_command_palette_entries(vec![
    ///         ContextMenuItem::new("app.open_file", "Open File"),
    ///     ]);
    /// assert_eq!(editor.custom_command_palette_entries().len(), 1);
    /// ```
    #[must_use]
    pub fn with_custom_command_palette_entries(
        mut self,
        entries: Vec<ContextMenuItem>,
    ) -> Self {
        self.set_custom_command_palette_entries(entries);
        self
    }

    /// Returns the custom command-palette entries in display order.
    ///
    /// # Returns
    ///
    /// The entries previously set, or an empty slice if none were set
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert!(editor.custom_command_palette_entries().is_empty());
    /// ```
    pub fn custom_command_palette_entries(&self) -> &[ContextMenuItem] {
        &self.custom_command_palette_entries
    }

    /// Sets whether the built-in editor commands are listed in the palette.
    ///
    /// Disabling them leaves only the custom entries, which is how a host
    /// application takes the palette over rather than extending it.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to list the built-in commands
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_default_command_palette_enabled(false);
    /// assert!(!editor.default_command_palette_enabled());
    /// ```
    pub fn set_default_command_palette_enabled(&mut self, enabled: bool) {
        self.default_command_palette_enabled = enabled;
    }

    /// Sets built-in command visibility using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to list the built-in commands
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_default_command_palette_enabled(false);
    /// assert!(!editor.default_command_palette_enabled());
    /// ```
    #[must_use]
    pub fn with_default_command_palette_enabled(
        mut self,
        enabled: bool,
    ) -> Self {
        self.set_default_command_palette_enabled(enabled);
        self
    }

    /// Sets whether the command palette can be opened.
    ///
    /// Turning it off makes `Ctrl/Cmd+Shift+P` and
    /// [`CodeEditor::open_command_palette`] no-ops, freeing the shortcut for
    /// a host application that provides its own palette.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether the palette can be opened
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_command_palette_enabled(false);
    /// assert!(!editor.command_palette_enabled());
    /// ```
    pub fn set_command_palette_enabled(&mut self, enabled: bool) {
        self.command_palette_enabled = enabled;
        if !enabled {
            self.command_palette_state.close();
        }
    }

    /// Sets whether the built-in reveal-in-file-manager action is shown.
    ///
    /// The action is only useful when the host application knows the file path
    /// backing the editor, so it is off by default and the host opts in.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to show the reveal action
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_reveal_in_file_manager_enabled(true);
    /// assert!(editor.reveal_in_file_manager_enabled());
    /// ```
    pub fn set_reveal_in_file_manager_enabled(&mut self, enabled: bool) {
        self.reveal_in_file_manager_enabled = enabled;
    }

    /// Sets reveal-in-file-manager visibility using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to show the reveal action
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_reveal_in_file_manager_enabled(true);
    /// assert!(editor.reveal_in_file_manager_enabled());
    /// ```
    #[must_use]
    pub fn with_reveal_in_file_manager_enabled(
        mut self,
        enabled: bool,
    ) -> Self {
        self.set_reveal_in_file_manager_enabled(enabled);
        self
    }

    /// Enables or disables Vim behavior for this editor instance.
    ///
    /// Changing this setting enters a clean Normal mode without modifying the
    /// buffer or command history. Any secondary cursors are collapsed onto the
    /// primary one, since Vim owns its own selection model.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable Vim behavior
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, VimMode};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_vim_enabled(true);
    ///
    /// // Vim always starts in Normal mode.
    /// assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
    /// ```
    pub fn set_vim_enabled(&mut self, enabled: bool) {
        if self.is_grouping {
            self.history.end_group();
            self.is_grouping = false;
        }
        self.vim_enabled = enabled;
        self.vim_state.enter_clean_normal_mode();
        self.cursors.remove_all_but_primary();
        let position = if enabled {
            self.vim_normal_position(self.cursors.primary_position())
        } else {
            self.cursors.primary_position()
        };
        self.cursors.set_single(position);
        self.is_dragging = false;
        self.overlay_cache.clear();
    }

    /// Returns the active Vim mode, or `None` when Vim behavior is disabled.
    ///
    /// Use this to drive a mode indicator in the host application's status bar.
    ///
    /// # Returns
    ///
    /// `Some(mode)` when Vim is enabled, `None` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, VimMode};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // `None` rather than a default mode, so a status bar can tell
    /// // "Vim is off" apart from "Vim is in Normal mode".
    /// assert_eq!(editor.vim_mode(), None);
    ///
    /// let editor = editor.with_vim_enabled(true);
    /// assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
    /// ```
    pub fn vim_mode(&self) -> Option<VimMode> {
        self.vim_enabled.then(|| self.vim_state.mode())
    }

    /// Sets the theme style for the editor.
    ///
    /// # Arguments
    ///
    /// * `style` - The style to apply to the editor
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, theme};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_theme(theme::from_iced_theme(&iced::Theme::TokyoNightStorm));
    /// ```
    pub fn set_theme(&mut self, style: Style) {
        self.style = style;
        // The highlight cache stores resolved colors, not scopes, and the
        // syntect palette is picked from the style's background lightness
        // (see `CodeEditor::resolve_syntax`). A theme change can therefore
        // flip light/dark and invalidate every cached span.
        self.invalidate_highlight_from(0);
        self.content_cache.clear();
        self.overlay_cache.clear();
    }

    /// Sets the language for UI translations.
    ///
    /// This changes the language used for all UI text elements in the editor,
    /// including search dialog tooltips, placeholders, and labels.
    ///
    /// # Arguments
    ///
    /// * `language` - The language to use for UI text
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Language};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_language(Language::French);
    /// ```
    pub fn set_language(&mut self, language: crate::i18n::Language) {
        self.translations.set_language(language);
        self.overlay_cache.clear();
    }

    /// Returns the current UI language.
    ///
    /// # Returns
    ///
    /// The currently active language for UI text
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, Language};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// let current_lang = editor.language();
    /// ```
    pub fn language(&self) -> crate::i18n::Language {
        self.translations.language()
    }

    /// Sets whether line wrapping is enabled.
    ///
    /// When enabled, long lines will wrap at the viewport width or at a
    /// configured column width.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable line wrapping
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_wrap_enabled(false); // Disable wrapping
    /// ```
    pub fn set_wrap_enabled(&mut self, enabled: bool) {
        if self.wrap_enabled != enabled {
            self.wrap_enabled = enabled;
            if enabled {
                self.horizontal_scroll_offset = 0.0;
            }
            self.content_cache.clear();
            self.overlay_cache.clear();
        }
    }

    /// Enables or disables visible whitespace rendering.
    ///
    /// When enabled, space characters are rendered as `·` and tab characters
    /// as `→`, both drawn in a dimmed color to remain non-intrusive. Toggling
    /// this setting clears the content cache to trigger an immediate redraw.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to show whitespace characters
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_show_whitespace(true);
    /// ```
    pub fn set_show_whitespace(&mut self, enabled: bool) {
        if self.show_whitespace != enabled {
            self.show_whitespace = enabled;
            self.content_cache.clear();
        }
    }

    /// Enables or disables indentation guides.
    ///
    /// Indentation guides are faint vertical lines drawn at every indentation
    /// level, making the nesting of a block visible at a glance. Their spacing
    /// follows [`CodeEditor::indent_style`]. Toggling this setting clears the
    /// content cache to trigger an immediate redraw.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to draw indentation guides
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_show_indent_guides(false);
    /// ```
    pub fn set_show_indent_guides(&mut self, enabled: bool) {
        if self.show_indent_guides != enabled {
            self.show_indent_guides = enabled;
            self.content_cache.clear();
        }
    }

    /// Enables or disables inline color previews.
    ///
    /// An inline color preview is a small square drawn just after a color
    /// literal — `#1e1e2e`, `0xFF6B6B`, `rgb(58, 123, 213)` — filled with the
    /// color the literal denotes, so a palette can be read without decoding
    /// hexadecimal by eye. Toggling this setting clears the content cache to
    /// trigger an immediate redraw.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to draw color-preview swatches
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("body { color: #f0c; }", "css");
    /// editor.set_show_color_previews(false);
    /// ```
    pub fn set_show_color_previews(&mut self, enabled: bool) {
        if self.show_color_previews != enabled {
            self.show_color_previews = enabled;
            self.content_cache.clear();
        }
    }

    /// Enables or disables sticky scroll.
    ///
    /// Sticky scroll pins the header lines of the blocks enclosing the topmost
    /// visible line to the top of the viewport, so the structural context stays
    /// readable while scrolling deep inside a long block. Clicking a pinned
    /// header scrolls back to it. Enclosing blocks are detected from
    /// indentation, exactly like code folding, so the feature is
    /// language-agnostic but follows the file's indentation rather than its
    /// syntax tree.
    ///
    /// Independent of [`CodeEditor::set_folding_enabled`], despite sharing that
    /// detection: turning code folding off removes the fold chevrons and leaves
    /// the pinned headers alone.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to pin enclosing block headers above the viewport
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {\n    let x = 1;\n}", "rs");
    /// editor.set_sticky_scroll_enabled(false);
    /// ```
    pub fn set_sticky_scroll_enabled(&mut self, enabled: bool) {
        self.sticky_scroll_enabled = enabled;
    }

    /// Enables or disables the matching-bracket/quote-pair highlight overlay.
    ///
    /// When enabled, placing the cursor next to a bracket (`(`, `)`, `[`,
    /// `]`, `{`, `}`) or a quote (`"`, `'`) highlights it and its matching
    /// pair. When disabled, no matching scan or highlight is performed.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable the bracket/quote-matching highlight
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_bracket_match_highlight_enabled(false); // Disable
    /// ```
    pub fn set_bracket_match_highlight_enabled(&mut self, enabled: bool) {
        if self.bracket_match_highlight_enabled != enabled {
            self.bracket_match_highlight_enabled = enabled;
            self.overlay_cache.clear();
        }
    }

    /// Enables or disables bracket-pair colorization (rainbow brackets).
    ///
    /// When enabled, each `( ) [ ] { }` is colored by its nesting depth, so a
    /// matching pair always shares the same color, cycling through a fixed
    /// palette as depth increases. When disabled, brackets render with their
    /// normal syntax-highlight color.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable bracket-pair colorization
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_bracket_pair_colorization_enabled(false); // Disable
    /// ```
    pub fn set_bracket_pair_colorization_enabled(&mut self, enabled: bool) {
        if self.bracket_pair_colorization_enabled != enabled {
            self.bracket_pair_colorization_enabled = enabled;
            self.content_cache.clear();
        }
    }

    /// Enables or disables code folding (collapse/expand blocks).
    ///
    /// When disabled, no fold chevrons are drawn and all lines are shown
    /// regardless of the collapsed state (which is preserved, so re-enabling
    /// restores the previously collapsed blocks).
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable code folding
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_folding_enabled(true);
    /// ```
    pub fn set_folding_enabled(&mut self, enabled: bool) {
        if self.folding_enabled != enabled {
            self.folding_enabled = enabled;
            self.bump_fold_revision();
        }
    }

    /// Enables or disables automatic indentation on Enter.
    ///
    /// When enabled, pressing Enter copies the leading whitespace of the
    /// current line to the new line. When disabled, the cursor is placed
    /// at column 0 on the new line.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` to enable auto-indentation, `false` to disable
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_auto_indent_enabled(false);
    /// assert!(!editor.auto_indent_enabled());
    /// ```
    pub fn set_auto_indent_enabled(&mut self, enabled: bool) {
        self.auto_indent_enabled = enabled;
    }

    /// Enables or disables auto-closing of brackets and quotes.
    ///
    /// When enabled, typing an opening bracket/quote (`(`, `[`, `{`, `"`,
    /// `'`) auto-inserts its matching closing character with the cursor
    /// placed between them, typing the closing character right before an
    /// already-inserted match moves the cursor past it instead of
    /// duplicating it, and typing an opening bracket/quote while text is
    /// selected wraps the selection in the pair instead of replacing it.
    ///
    /// # Arguments
    ///
    /// * `enabled` - `true` to enable auto-closing, `false` to disable
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_auto_close_brackets(false); // Disable auto-closing
    /// ```
    pub fn set_auto_close_brackets(&mut self, enabled: bool) {
        self.auto_close_brackets = enabled;
    }

    /// Sets the indentation style used when pressing the Tab key.
    ///
    /// # Arguments
    ///
    /// * `style` - The indentation style (`IndentStyle::Spaces(n)` or `IndentStyle::Tab`)
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, IndentStyle};
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    ///
    /// // Indent with a real tab character instead of the default 4 spaces.
    /// editor.set_indent_style(IndentStyle::Tab);
    /// assert_eq!(editor.indent_style(), IndentStyle::Tab);
    ///
    /// // Or pick a different space width.
    /// editor.set_indent_style(IndentStyle::Spaces(2));
    /// assert_eq!(editor.indent_style(), IndentStyle::Spaces(2));
    /// ```
    pub fn set_indent_style(&mut self, style: IndentStyle) {
        self.indent_style = style;
    }

    /// Returns the current indentation style.
    ///
    /// # Returns
    ///
    /// The current [`IndentStyle`] configured for this editor
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::{CodeEditor, IndentStyle};
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert_eq!(editor.indent_style(), IndentStyle::Spaces(4));
    /// ```
    pub fn indent_style(&self) -> IndentStyle {
        self.indent_style
    }

    /// Enables or disables the search/replace functionality.
    ///
    /// When disabled, search/replace keyboard shortcuts (Ctrl+F, Ctrl+H, F3)
    /// will be ignored. If the search dialog is currently open, it will be closed.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable search/replace functionality
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_search_replace_enabled(false); // Disable search/replace
    /// ```
    pub fn set_search_replace_enabled(&mut self, enabled: bool) {
        self.search_replace_enabled = enabled;
        if !enabled && self.search_state.is_open {
            self.search_state.close();
        }
    }

    /// Returns the syntax highlighting language identifier for this editor.
    ///
    /// This is the language key passed at construction (e.g., `"lua"`, `"rs"`, `"py"`).
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert_eq!(editor.syntax(), "rs");
    /// ```
    pub fn syntax(&self) -> &str {
        &self.syntax
    }

    /// Returns the display name of the grammar actually used to highlight
    /// this editor's content.
    ///
    /// [`CodeEditor::syntax`] returns the raw identifier the host set; this
    /// returns what the highlighter resolved it to, so it also reports the
    /// plain-text fallback when no grammar matches. It is the value to show in
    /// a status bar: it tells the user which language the editor is really
    /// coloring with, not which one was requested.
    ///
    /// # Returns
    ///
    /// The grammar's name (e.g. `"Rust"`, `"Lua"`, `"Plain Text"`).
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// assert_eq!(editor.syntax_name(), "Rust");
    ///
    /// // An identifier no bundled grammar matches degrades to plain text.
    /// let unknown = CodeEditor::new("...", "no-such-language");
    /// assert_eq!(unknown.syntax_name(), "Plain Text");
    /// ```
    pub fn syntax_name(&self) -> &'static str {
        let (_, syntax, _) = self.resolve_syntax();
        syntax.map_or("Plain Text", |syntax| syntax.name.as_str())
    }

    /// Sets the syntax highlighting language identifier for this editor.
    ///
    /// Use this when the content a single editor shows changes language --
    /// typically after opening or saving a file under a different extension.
    /// The identifier is the same key [`CodeEditor::new`] takes: a file
    /// extension (`"rs"`, `"py"`, `"toml"`) or one of the aliases
    /// `resolve_syntax` normalizes (`"rust"`, `"python"`, `"markdown"`, ...).
    /// An unknown identifier falls back to plain text rather than losing the
    /// content.
    ///
    /// Changing the identifier drops the cached per-line highlighting, so the
    /// visible lines are re-tokenized on the next render. Setting the current
    /// identifier again does nothing.
    ///
    /// # Arguments
    ///
    /// * `syntax` - Syntax highlighting language identifier (e.g. `"rs"`)
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("-- comment", "lua");
    /// assert_eq!(editor.syntax(), "lua");
    ///
    /// editor.set_syntax("rs");
    /// assert_eq!(editor.syntax(), "rs");
    /// ```
    pub fn set_syntax(&mut self, syntax: &str) {
        if self.syntax == syntax {
            return;
        }

        self.syntax.clear();
        self.syntax.push_str(syntax);
        // `highlighted_line_cached` rebuilds `HighlightCache` on its own when
        // the active syntax no longer matches, so only the canvas needs an
        // explicit redraw here.
        self.content_cache.clear();
    }

    /// Sets the wrap column (fixed width wrapping).
    ///
    /// When set to `Some(n)`, lines will wrap at column `n`.
    /// When set to `None`, lines will wrap at the viewport width.
    ///
    /// # Arguments
    ///
    /// * `column` - The column to wrap at, or None for viewport-based wrapping
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_wrap_column(Some(80)); // Wrap at 80 characters
    /// ```
    #[must_use]
    pub fn with_wrap_column(mut self, column: Option<usize>) -> Self {
        self.wrap_column = column;
        self
    }

    /// Sets whether line numbers are displayed.
    ///
    /// When disabled, the gutter is completely removed (0px width),
    /// providing more space for code display.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to display line numbers
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// editor.set_line_numbers_enabled(false); // Hide line numbers
    /// ```
    pub fn set_line_numbers_enabled(&mut self, enabled: bool) {
        if self.line_numbers_enabled != enabled {
            self.line_numbers_enabled = enabled;
            self.content_cache.clear();
            self.overlay_cache.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::features::context_menu::ContextMenuItem;
    use crate::canvas_editor::features::vim;

    #[test]
    fn test_custom_context_menu_configuration() {
        let custom_entries = vec![
            ContextMenuEntry::item("format", "Format document")
                .with_shortcut("Shift+Alt+F"),
            ContextMenuEntry::separator(),
            ContextMenuEntry::Item(
                ContextMenuItem::new("rename", "Rename symbol")
                    .with_enabled(false),
            ),
        ];

        let editor = CodeEditor::new("", "rs")
            .with_custom_context_menu_entries(custom_entries.clone())
            .with_default_context_menu_enabled(false);

        assert_eq!(editor.custom_context_menu_entries(), custom_entries);
        assert!(!editor.default_context_menu_enabled());

        let default_editor = CodeEditor::new("", "rs");
        assert!(default_editor.custom_context_menu_entries().is_empty());
        assert!(default_editor.default_context_menu_enabled());
    }

    #[test]
    fn test_reveal_in_file_manager_configuration() {
        let mut editor = CodeEditor::new("", "rs");
        assert!(!editor.reveal_in_file_manager_enabled());

        editor.set_reveal_in_file_manager_enabled(true);
        assert!(editor.reveal_in_file_manager_enabled());

        let editor =
            CodeEditor::new("", "rs").with_reveal_in_file_manager_enabled(true);
        assert!(editor.reveal_in_file_manager_enabled());
    }

    #[test]
    fn test_auto_close_brackets_configuration() {
        let mut editor = CodeEditor::new("", "rs");
        assert!(editor.auto_close_brackets());

        editor.set_auto_close_brackets(false);
        assert!(!editor.auto_close_brackets());

        editor.set_auto_close_brackets(true);
        assert!(editor.auto_close_brackets());
    }

    #[test]
    fn test_bracket_match_highlight_configuration() {
        let mut editor = CodeEditor::new("", "rs");
        assert!(editor.bracket_match_highlight_enabled());

        editor.set_bracket_match_highlight_enabled(false);
        assert!(!editor.bracket_match_highlight_enabled());

        editor.set_bracket_match_highlight_enabled(true);
        assert!(editor.bracket_match_highlight_enabled());
    }

    #[test]
    fn test_bracket_pair_colorization_configuration() {
        let mut editor = CodeEditor::new("", "rs");
        assert!(editor.bracket_pair_colorization_enabled());

        editor.set_bracket_pair_colorization_enabled(false);
        assert!(!editor.bracket_pair_colorization_enabled());

        editor.set_bracket_pair_colorization_enabled(true);
        assert!(editor.bracket_pair_colorization_enabled());
    }

    #[test]
    fn test_show_indent_guides_configuration() {
        let mut editor = CodeEditor::new("", "rs");
        assert!(editor.show_indent_guides());

        editor.set_show_indent_guides(false);
        assert!(!editor.show_indent_guides());

        editor.set_show_indent_guides(true);
        assert!(editor.show_indent_guides());

        let editor = CodeEditor::new("", "rs").with_show_indent_guides(false);
        assert!(!editor.show_indent_guides());
    }

    #[test]
    fn test_show_color_previews_configuration() {
        let mut editor = CodeEditor::new("", "rs");
        assert!(editor.show_color_previews());

        editor.set_show_color_previews(false);
        assert!(!editor.show_color_previews());

        editor.set_show_color_previews(true);
        assert!(editor.show_color_previews());

        let editor = CodeEditor::new("", "rs").with_show_color_previews(false);
        assert!(!editor.show_color_previews());
    }

    #[test]
    fn vim_disabled_by_default() {
        let editor = CodeEditor::new("unchanged", "rs");

        assert!(!editor.vim_enabled());
        assert_eq!(editor.vim_mode(), None);
        assert_eq!(editor.content(), "unchanged");
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn vim_enable_enters_clean_normal_mode() {
        let mut editor = CodeEditor::new("unchanged", "rs");
        assert_eq!(editor.vim_state.parse_key('9'), None);
        assert_eq!(editor.vim_state.parse_key('d'), None);

        editor.set_vim_enabled(true);

        assert!(editor.vim_enabled());
        assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
        assert_eq!(
            editor.vim_state.parse_key('l'),
            Some(vim::VimAction::Motion {
                motion: vim::VimMotion::Right,
                count: 1,
                explicit_count: false,
            })
        );
        assert_eq!(editor.content(), "unchanged");
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn vim_disable_clears_pending_state() {
        let mut editor =
            CodeEditor::new("unchanged", "rs").with_vim_enabled(true);
        assert_eq!(editor.vim_state.parse_key('4'), None);
        assert_eq!(editor.vim_state.parse_key('d'), None);

        editor.set_vim_enabled(false);
        assert!(!editor.vim_enabled());
        assert_eq!(editor.vim_mode(), None);

        editor.set_vim_enabled(true);
        assert_eq!(
            editor.vim_state.parse_key('w'),
            Some(vim::VimAction::Motion {
                motion: vim::VimMotion::WordForward,
                count: 1,
                explicit_count: false,
            })
        );
        assert_eq!(editor.content(), "unchanged");
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn vim_reset_clears_pending_state() {
        let mut editor = CodeEditor::new("before", "rs").with_vim_enabled(true);
        assert_eq!(editor.vim_state.parse_key('3'), None);
        assert_eq!(editor.vim_state.parse_key('g'), None);

        let _ = editor.reset("after");

        assert_eq!(editor.vim_mode(), Some(VimMode::Normal));
        assert_eq!(editor.vim_state.parse_key('g'), None);
        assert_eq!(
            editor.vim_state.parse_key('g'),
            Some(vim::VimAction::Motion {
                motion: vim::VimMotion::DocumentStart,
                count: 1,
                explicit_count: false,
            })
        );
        assert_eq!(editor.content(), "after");
        assert!(!editor.can_undo());
        assert!(!editor.can_redo());
    }

    #[test]
    fn test_syntax_getter() {
        let editor = CodeEditor::new("", "lua");
        assert_eq!(editor.syntax(), "lua");
    }

    #[test]
    fn test_syntax_name_resolves_grammar_and_fallback() {
        assert_eq!(CodeEditor::new("", "rs").syntax_name(), "Rust");
        assert_eq!(CodeEditor::new("", "lua").syntax_name(), "Lua");
        // Aliases normalized by `resolve_syntax` resolve to the same grammar.
        assert_eq!(CodeEditor::new("", "rust").syntax_name(), "Rust");
        // Unknown identifiers degrade rather than losing the content.
        assert_eq!(
            CodeEditor::new("", "no-such-language").syntax_name(),
            "Plain Text"
        );
    }

    #[test]
    fn test_syntax_name_follows_set_syntax() {
        let mut editor = CodeEditor::new("", "lua");
        assert_eq!(editor.syntax_name(), "Lua");

        editor.set_syntax("py");
        assert_eq!(editor.syntax_name(), "Python");
    }

    #[test]
    fn test_set_syntax_updates_identifier() {
        let mut editor = CodeEditor::new("-- comment", "lua");

        editor.set_syntax("rs");
        assert_eq!(editor.syntax(), "rs");

        // Setting the same identifier again is a no-op, not an error.
        editor.set_syntax("rs");
        assert_eq!(editor.syntax(), "rs");
    }

    /// Color of the first highlighted span of the editor's first line, or
    /// `None` when syntect ships no definition for the active syntax.
    fn first_span_color(editor: &CodeEditor) -> Option<iced::Color> {
        let (syntax_set, syntax, theme) = editor.resolve_syntax();
        editor
            .highlighted_line_cached(0, syntax?, theme?, syntax_set)
            .first()
            .map(|(color, _)| *color)
    }

    #[test]
    fn test_set_syntax_rehighlights_with_the_new_language() {
        // A Rust doc comment: a comment under `rs`, plain code under `lua`.
        let mut editor = CodeEditor::new("/// doc", "lua");

        let as_lua = first_span_color(&editor);
        assert!(as_lua.is_some(), "syntect ships a Lua definition");

        editor.set_syntax("rs");
        let as_rust = first_span_color(&editor);
        assert!(as_rust.is_some(), "syntect ships a Rust definition");

        assert_ne!(
            as_lua, as_rust,
            "`/// doc` must not keep its Lua colors after switching to Rust"
        );
    }

    #[test]
    fn test_set_theme_rehighlights_for_the_new_palette() {
        let mut editor = CodeEditor::new("// comment", "rs");

        editor.set_theme(crate::theme::from_iced_theme(&iced::Theme::Dark));
        let dark = first_span_color(&editor);
        assert!(dark.is_some(), "syntect ships a Rust definition");

        editor.set_theme(crate::theme::from_iced_theme(&iced::Theme::Light));
        let light = first_span_color(&editor);

        assert_ne!(
            dark, light,
            "the cached dark colors must be dropped when the theme turns light"
        );
    }

    #[test]
    fn test_folding_enabled_by_default() {
        let editor = CodeEditor::new("fn main() {}", "rs");
        assert!(editor.folding_enabled());
    }
}
