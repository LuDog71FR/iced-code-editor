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

    /// Sets built-in context-menu visibility using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to show the built-in actions
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
    ///     .with_default_context_menu_enabled(false);
    /// assert!(!editor.default_context_menu_enabled());
    /// ```
    #[must_use]
    pub fn with_default_context_menu_enabled(mut self, enabled: bool) -> Self {
        self.set_default_context_menu_enabled(enabled);
        self
    }

    /// Returns whether the built-in context-menu actions are enabled.
    ///
    /// # Returns
    ///
    /// `true` if the built-in actions are shown, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Shown by default.
    /// assert!(editor.default_context_menu_enabled());
    /// ```
    pub fn default_context_menu_enabled(&self) -> bool {
        self.default_context_menu_enabled
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

    /// Returns whether the built-in commands are listed in the palette.
    ///
    /// # Returns
    ///
    /// `true` if the built-in commands are listed, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Listed by default.
    /// assert!(editor.default_command_palette_enabled());
    /// ```
    pub fn default_command_palette_enabled(&self) -> bool {
        self.default_command_palette_enabled
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

    /// Sets command-palette availability using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether the palette can be opened
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
    ///     .with_command_palette_enabled(false);
    /// assert!(!editor.command_palette_enabled());
    /// ```
    #[must_use]
    pub fn with_command_palette_enabled(mut self, enabled: bool) -> Self {
        self.set_command_palette_enabled(enabled);
        self
    }

    /// Returns whether the command palette can be opened.
    ///
    /// # Returns
    ///
    /// `true` if the palette can be opened, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Available by default.
    /// assert!(editor.command_palette_enabled());
    /// ```
    pub fn command_palette_enabled(&self) -> bool {
        self.command_palette_enabled
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

    /// Returns whether the reveal-in-file-manager action is shown.
    ///
    /// # Returns
    ///
    /// `true` if the reveal action is shown, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Off by default: the editor doesn't know its own file path.
    /// assert!(!editor.reveal_in_file_manager_enabled());
    /// ```
    pub fn reveal_in_file_manager_enabled(&self) -> bool {
        self.reveal_in_file_manager_enabled
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

    /// Sets whether Vim behavior is enabled using the builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable Vim behavior
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
    ///     .with_vim_enabled(true);
    /// assert!(editor.vim_enabled());
    /// ```
    #[must_use]
    pub fn with_vim_enabled(mut self, enabled: bool) -> Self {
        self.set_vim_enabled(enabled);
        self
    }

    /// Returns whether Vim behavior is enabled for this editor instance.
    ///
    /// # Returns
    ///
    /// `true` if Vim behavior is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Off by default.
    /// assert!(!editor.vim_enabled());
    /// ```
    pub fn vim_enabled(&self) -> bool {
        self.vim_enabled
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

    /// Returns whether line wrapping is enabled.
    ///
    /// # Returns
    ///
    /// `true` if line wrapping is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.wrap_enabled());
    ///
    /// editor.set_wrap_enabled(false);
    /// assert!(!editor.wrap_enabled());
    /// ```
    pub fn wrap_enabled(&self) -> bool {
        self.wrap_enabled
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

    /// Returns whether visible whitespace rendering is enabled.
    ///
    /// # Returns
    ///
    /// `true` if whitespace characters are rendered, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.show_whitespace());
    ///
    /// editor.set_show_whitespace(false);
    /// assert!(!editor.show_whitespace());
    /// ```
    pub fn show_whitespace(&self) -> bool {
        self.show_whitespace
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

    /// Returns whether indentation guides are drawn.
    ///
    /// # Returns
    ///
    /// `true` if indentation guides are rendered, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.show_indent_guides());
    ///
    /// editor.set_show_indent_guides(false);
    /// assert!(!editor.show_indent_guides());
    /// ```
    pub fn show_indent_guides(&self) -> bool {
        self.show_indent_guides
    }

    /// Sets the indentation-guide display with builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to draw indentation guides
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
    ///     .with_show_indent_guides(false);
    /// assert!(!editor.show_indent_guides());
    /// ```
    #[must_use]
    pub fn with_show_indent_guides(mut self, enabled: bool) -> Self {
        self.show_indent_guides = enabled;
        self
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

    /// Returns whether inline color previews are drawn.
    ///
    /// # Returns
    ///
    /// `true` if color-preview swatches are rendered, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("body { color: #f0c; }", "css");
    /// // Enabled by default.
    /// assert!(editor.show_color_previews());
    ///
    /// editor.set_show_color_previews(false);
    /// assert!(!editor.show_color_previews());
    /// ```
    pub fn show_color_previews(&self) -> bool {
        self.show_color_previews
    }

    /// Sets the inline color-preview display with builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to draw color-preview swatches
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
    /// let editor = CodeEditor::new("body { color: #f0c; }", "css")
    ///     .with_show_color_previews(false);
    /// assert!(!editor.show_color_previews());
    /// ```
    #[must_use]
    pub fn with_show_color_previews(mut self, enabled: bool) -> Self {
        self.show_color_previews = enabled;
        self
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

    /// Returns whether the matching-bracket/quote-pair highlight overlay is enabled.
    ///
    /// # Returns
    ///
    /// `true` if the highlight is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.bracket_match_highlight_enabled());
    ///
    /// editor.set_bracket_match_highlight_enabled(false);
    /// assert!(!editor.bracket_match_highlight_enabled());
    /// ```
    pub fn bracket_match_highlight_enabled(&self) -> bool {
        self.bracket_match_highlight_enabled
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

    /// Returns whether bracket-pair colorization (rainbow brackets) is enabled.
    ///
    /// # Returns
    ///
    /// `true` if bracket-pair colorization is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.bracket_pair_colorization_enabled());
    ///
    /// editor.set_bracket_pair_colorization_enabled(false);
    /// assert!(!editor.bracket_pair_colorization_enabled());
    /// ```
    pub fn bracket_pair_colorization_enabled(&self) -> bool {
        self.bracket_pair_colorization_enabled
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

    /// Returns whether code folding is enabled.
    ///
    /// # Returns
    ///
    /// `true` if code folding is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.folding_enabled());
    ///
    /// editor.set_folding_enabled(false);
    /// assert!(!editor.folding_enabled());
    /// ```
    pub fn folding_enabled(&self) -> bool {
        self.folding_enabled
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

    /// Returns whether auto-indentation is enabled.
    ///
    /// # Returns
    ///
    /// `true` if auto-indentation is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.auto_indent_enabled());
    /// ```
    pub fn auto_indent_enabled(&self) -> bool {
        self.auto_indent_enabled
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

    /// Returns whether auto-closing of brackets and quotes is enabled.
    ///
    /// # Returns
    ///
    /// `true` if auto-closing is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.auto_close_brackets());
    /// ```
    pub fn auto_close_brackets(&self) -> bool {
        self.auto_close_brackets
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

    /// Returns whether search/replace functionality is enabled.
    ///
    /// # Returns
    ///
    /// `true` if search/replace is enabled, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Enabled by default.
    /// assert!(editor.search_replace_enabled());
    ///
    /// editor.set_search_replace_enabled(false);
    /// assert!(!editor.search_replace_enabled());
    /// ```
    pub fn search_replace_enabled(&self) -> bool {
        self.search_replace_enabled
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

    /// Sets the line wrapping with builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable line wrapping
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
    ///     .with_wrap_enabled(false);
    /// ```
    #[must_use]
    pub fn with_wrap_enabled(mut self, enabled: bool) -> Self {
        self.wrap_enabled = enabled;
        self
    }

    /// Enables or disables code folding using the builder pattern.
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
    /// let editor = CodeEditor::new("fn main() {}", "rs")
    ///     .with_folding_enabled(true);
    /// ```
    #[must_use]
    pub fn with_folding_enabled(mut self, enabled: bool) -> Self {
        self.folding_enabled = enabled;
        self
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

    /// Returns whether line numbers are displayed.
    ///
    /// # Returns
    ///
    /// `true` if line numbers are displayed, `false` otherwise
    ///
    /// # Example
    ///
    /// ```
    /// use iced_code_editor::CodeEditor;
    ///
    /// let mut editor = CodeEditor::new("fn main() {}", "rs");
    /// // Displayed by default.
    /// assert!(editor.line_numbers_enabled());
    ///
    /// editor.set_line_numbers_enabled(false);
    /// assert!(!editor.line_numbers_enabled());
    /// ```
    pub fn line_numbers_enabled(&self) -> bool {
        self.line_numbers_enabled
    }

    /// Sets the line numbers display with builder pattern.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to display line numbers
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
    ///     .with_line_numbers_enabled(false);
    /// ```
    #[must_use]
    pub fn with_line_numbers_enabled(mut self, enabled: bool) -> Self {
        self.line_numbers_enabled = enabled;
        self
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
    fn test_folding_enabled_by_default() {
        let editor = CodeEditor::new("fn main() {}", "rs");
        assert!(editor.folding_enabled());
    }
}
