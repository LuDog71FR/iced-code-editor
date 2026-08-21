//! Internationalization support for the code editor.
//!
//! This module provides translation support for UI text in the search dialog.
//!
//! # Using `Translations`
//!
//! The translations are available in YAML files in the `locales` directory,
//! loaded through the `rust-i18n` crate. Application code does not call
//! `rust-i18n`'s `t!` macro directly — it is not part of this crate's public
//! API. Instead, look up translated strings through [`Translations`]:
//!
//! ```
//! use iced_code_editor::{Language, Translations};
//!
//! let translations = Translations::new(Language::English);
//! assert_eq!(translations.search_placeholder(), "Search...");
//! ```

/// Supported languages for the code editor UI.
///
/// # Examples
///
/// ```
/// use iced_code_editor::Language;
///
/// let lang = Language::English;
/// assert_eq!(lang, Language::default());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// English language
    #[default]
    English,
    /// French language
    French,
    /// Spanish language
    Spanish,
    /// German language
    German,
    /// Italian language
    Italian,
    /// Portuguese (Brazilian) language
    PortugueseBR,
    /// Portuguese (European) language
    PortuguesePT,
    /// Simplified Chinese language
    ChineseSimplified,
}

impl Language {
    /// Returns the locale code for this language.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::Language;
    ///
    /// assert_eq!(Language::English.to_locale(), "en");
    /// assert_eq!(Language::French.to_locale(), "fr");
    /// assert_eq!(Language::Spanish.to_locale(), "es");
    /// assert_eq!(Language::German.to_locale(), "de");
    /// assert_eq!(Language::Italian.to_locale(), "it");
    /// assert_eq!(Language::PortugueseBR.to_locale(), "pt-BR");
    /// assert_eq!(Language::PortuguesePT.to_locale(), "pt-PT");
    /// ```
    #[must_use]
    pub const fn to_locale(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
            Self::Spanish => "es",
            Self::German => "de",
            Self::Italian => "it",
            Self::PortugueseBR => "pt-BR",
            Self::PortuguesePT => "pt-PT",
            Self::ChineseSimplified => "zh-CN",
        }
    }
}

/// Provides translated text strings for UI elements.
///
/// This struct contains all UI text translations used in the search dialog,
/// including placeholders, tooltips, and labels.
///
/// # Examples
///
/// ```
/// use iced_code_editor::{Language, Translations};
///
/// let translations = Translations::new(Language::French);
/// assert_eq!(translations.search_placeholder(), "Rechercher...");
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct Translations {
    language: Language,
}

impl Translations {
    /// Creates a new `Translations` instance with the specified language.
    ///
    /// This sets the global rust-i18n locale to the specified language.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let translations = Translations::new(Language::Spanish);
    /// assert_eq!(translations.language(), Language::Spanish);
    /// ```
    #[must_use]
    pub fn new(language: Language) -> Self {
        rust_i18n::set_locale(language.to_locale());
        Self { language }
    }

    /// Returns the current language.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let translations = Translations::new(Language::French);
    /// assert_eq!(translations.language(), Language::French);
    /// ```
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    /// Sets the language for translations.
    ///
    /// This updates the global rust-i18n locale.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let mut translations = Translations::new(Language::English);
    /// translations.set_language(Language::Spanish);
    /// assert_eq!(translations.language(), Language::Spanish);
    /// ```
    pub fn set_language(&mut self, language: Language) {
        self.language = language;
        rust_i18n::set_locale(language.to_locale());
    }

    /// Returns the placeholder text for the search input field.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.search_placeholder(), "Search...");
    ///
    /// let fr = Translations::new(Language::French);
    /// assert_eq!(fr.search_placeholder(), "Rechercher...");
    /// ```
    #[must_use]
    pub fn search_placeholder(&self) -> String {
        rust_i18n::t!("search.placeholder", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the placeholder text for the replace input field.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let es = Translations::new(Language::Spanish);
    /// assert_eq!(es.replace_placeholder(), "Reemplazar...");
    /// ```
    #[must_use]
    pub fn replace_placeholder(&self) -> String {
        rust_i18n::t!("replace.placeholder", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the label text for the case sensitive checkbox.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let fr = Translations::new(Language::French);
    /// assert_eq!(fr.case_sensitive_label(), "Sensible à la casse");
    /// ```
    #[must_use]
    pub fn case_sensitive_label(&self) -> String {
        rust_i18n::t!(
            "settings.case_sensitive_label",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the tooltip text for the previous match button.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.previous_match_tooltip(), "Previous match (Shift+F3)");
    /// ```
    #[must_use]
    pub fn previous_match_tooltip(&self) -> String {
        rust_i18n::t!(
            "search.previous_match_tooltip",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the tooltip text for the next match button.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let es = Translations::new(Language::Spanish);
    /// assert_eq!(es.next_match_tooltip(), "Siguiente coincidencia (F3 / Enter)");
    /// ```
    #[must_use]
    pub fn next_match_tooltip(&self) -> String {
        rust_i18n::t!(
            "search.next_match_tooltip",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the tooltip text for the close search dialog button.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let fr = Translations::new(Language::French);
    /// assert_eq!(fr.close_search_tooltip(), "Fermer la recherche (Échap)");
    /// ```
    #[must_use]
    pub fn close_search_tooltip(&self) -> String {
        rust_i18n::t!(
            "search.close_tooltip",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the tooltip text for the replace current match button.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.replace_current_tooltip(), "Replace current match");
    /// ```
    #[must_use]
    pub fn replace_current_tooltip(&self) -> String {
        rust_i18n::t!(
            "replace.current_tooltip",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the tooltip text for the replace all matches button.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let es = Translations::new(Language::Spanish);
    /// assert_eq!(es.replace_all_tooltip(), "Reemplazar todo");
    /// ```
    #[must_use]
    pub fn replace_all_tooltip(&self) -> String {
        rust_i18n::t!("replace.all_tooltip", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the context-menu label for undo.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_undo(), "Undo");
    ///
    /// let fr = Translations::new(Language::French);
    /// assert_eq!(fr.context_menu_undo(), "Annuler");
    /// ```
    #[must_use]
    pub fn context_menu_undo(&self) -> String {
        rust_i18n::t!("context_menu.undo", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the context-menu label for redo.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_redo(), "Redo");
    ///
    /// let fr = Translations::new(Language::French);
    /// assert_eq!(fr.context_menu_redo(), "Rétablir");
    /// ```
    #[must_use]
    pub fn context_menu_redo(&self) -> String {
        rust_i18n::t!("context_menu.redo", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the context-menu label for cut.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_cut(), "Cut");
    ///
    /// let de = Translations::new(Language::German);
    /// assert_eq!(de.context_menu_cut(), "Ausschneiden");
    /// ```
    #[must_use]
    pub fn context_menu_cut(&self) -> String {
        rust_i18n::t!("context_menu.cut", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the context-menu label for copy.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_copy(), "Copy");
    ///
    /// let it = Translations::new(Language::Italian);
    /// assert_eq!(it.context_menu_copy(), "Copia");
    /// ```
    #[must_use]
    pub fn context_menu_copy(&self) -> String {
        rust_i18n::t!("context_menu.copy", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the context-menu label for paste.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_paste(), "Paste");
    ///
    /// let es = Translations::new(Language::Spanish);
    /// assert_eq!(es.context_menu_paste(), "Pegar");
    /// ```
    #[must_use]
    pub fn context_menu_paste(&self) -> String {
        rust_i18n::t!("context_menu.paste", locale = self.language.to_locale())
            .into_owned()
    }

    /// Returns the context-menu label for select all.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_select_all(), "Select All");
    ///
    /// let fr = Translations::new(Language::French);
    /// assert_eq!(fr.context_menu_select_all(), "Tout sélectionner");
    /// ```
    #[must_use]
    pub fn context_menu_select_all(&self) -> String {
        rust_i18n::t!(
            "context_menu.select_all",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the macOS context-menu label for revealing a file.
    ///
    /// Prefer [`Self::context_menu_reveal_in_file_manager`], which picks the
    /// right one of the three platform wordings for you.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.context_menu_reveal_in_finder(), "Reveal in Finder");
    /// ```
    #[must_use]
    pub fn context_menu_reveal_in_finder(&self) -> String {
        rust_i18n::t!(
            "context_menu.reveal_in_finder",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the Windows context-menu label for revealing a file.
    ///
    /// Prefer [`Self::context_menu_reveal_in_file_manager`], which picks the
    /// right one of the three platform wordings for you.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(
    ///     en.context_menu_reveal_in_file_explorer(),
    ///     "Reveal in File Explorer"
    /// );
    /// ```
    #[must_use]
    pub fn context_menu_reveal_in_file_explorer(&self) -> String {
        rust_i18n::t!(
            "context_menu.reveal_in_file_explorer",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the Linux context-menu label for opening a file's parent folder.
    ///
    /// Worded as "open the containing folder" rather than "reveal", matching
    /// how Linux file managers describe the action.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(
    ///     en.context_menu_open_containing_folder(),
    ///     "Open Containing Folder"
    /// );
    /// ```
    #[must_use]
    pub fn context_menu_open_containing_folder(&self) -> String {
        rust_i18n::t!(
            "context_menu.open_containing_folder",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the platform-appropriate label for revealing a file.
    ///
    /// Resolves to "Reveal in Finder" on macOS, "Reveal in File Explorer" on
    /// Windows, and "Open Containing Folder" elsewhere, so the menu matches
    /// the wording users already know from their own file manager.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// let label = en.context_menu_reveal_in_file_manager();
    ///
    /// // Whichever platform this runs on, it matches that platform's wording.
    /// #[cfg(target_os = "macos")]
    /// assert_eq!(label, en.context_menu_reveal_in_finder());
    /// #[cfg(target_os = "windows")]
    /// assert_eq!(label, en.context_menu_reveal_in_file_explorer());
    /// #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    /// assert_eq!(label, en.context_menu_open_containing_folder());
    /// ```
    #[must_use]
    pub fn context_menu_reveal_in_file_manager(&self) -> String {
        if cfg!(target_os = "macos") {
            self.context_menu_reveal_in_finder()
        } else if cfg!(target_os = "windows") {
            self.context_menu_reveal_in_file_explorer()
        } else {
            self.context_menu_open_containing_folder()
        }
    }
    /// Returns the command-palette label for the filter input's placeholder.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_placeholder(), "Type a command...");
    /// ```
    #[must_use]
    pub fn command_palette_placeholder(&self) -> String {
        rust_i18n::t!(
            "command_palette.placeholder",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for the empty-result message.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_no_results(), "No matching command");
    /// ```
    #[must_use]
    pub fn command_palette_no_results(&self) -> String {
        rust_i18n::t!(
            "command_palette.no_results",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for saving the document.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_save(), "Save");
    /// ```
    #[must_use]
    pub fn command_palette_save(&self) -> String {
        rust_i18n::t!(
            "command_palette.save",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for toggling the line comment.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_toggle_comment(), "Toggle Line Comment");
    /// ```
    #[must_use]
    pub fn command_palette_toggle_comment(&self) -> String {
        rust_i18n::t!(
            "command_palette.toggle_comment",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for moving the current line up.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_move_line_up(), "Move Line Up");
    /// ```
    #[must_use]
    pub fn command_palette_move_line_up(&self) -> String {
        rust_i18n::t!(
            "command_palette.move_line_up",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for moving the current line down.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_move_line_down(), "Move Line Down");
    /// ```
    #[must_use]
    pub fn command_palette_move_line_down(&self) -> String {
        rust_i18n::t!(
            "command_palette.move_line_down",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for duplicating the current line above.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_duplicate_line_up(), "Duplicate Line Up");
    /// ```
    #[must_use]
    pub fn command_palette_duplicate_line_up(&self) -> String {
        rust_i18n::t!(
            "command_palette.duplicate_line_up",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for duplicating the current line below.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_duplicate_line_down(), "Duplicate Line Down");
    /// ```
    #[must_use]
    pub fn command_palette_duplicate_line_down(&self) -> String {
        rust_i18n::t!(
            "command_palette.duplicate_line_down",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for jumping to a line.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_goto_line(), "Go to Line");
    /// ```
    #[must_use]
    pub fn command_palette_goto_line(&self) -> String {
        rust_i18n::t!(
            "command_palette.goto_line",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for adding a cursor on the line above.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_add_cursor_above(), "Add Cursor Above");
    /// ```
    #[must_use]
    pub fn command_palette_add_cursor_above(&self) -> String {
        rust_i18n::t!(
            "command_palette.add_cursor_above",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for adding a cursor on the line below.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_add_cursor_below(), "Add Cursor Below");
    /// ```
    #[must_use]
    pub fn command_palette_add_cursor_below(&self) -> String {
        rust_i18n::t!(
            "command_palette.add_cursor_below",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for selecting the next occurrence.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_select_next_occurrence(), "Select Next Occurrence");
    /// ```
    #[must_use]
    pub fn command_palette_select_next_occurrence(&self) -> String {
        rust_i18n::t!(
            "command_palette.select_next_occurrence",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for toggling Vim mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_toggle_vim_mode(), "Toggle Vim Mode");
    /// ```
    #[must_use]
    pub fn command_palette_toggle_vim_mode(&self) -> String {
        rust_i18n::t!(
            "command_palette.toggle_vim_mode",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for opening the search dialog.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_find(), "Find");
    /// ```
    #[must_use]
    pub fn command_palette_find(&self) -> String {
        rust_i18n::t!(
            "command_palette.find",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for opening the search-and-replace dialog.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_replace(), "Replace");
    /// ```
    #[must_use]
    pub fn command_palette_replace(&self) -> String {
        rust_i18n::t!(
            "command_palette.replace",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for folding the block at the cursor.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_fold_at_cursor(), "Toggle Fold at Cursor");
    /// ```
    #[must_use]
    pub fn command_palette_fold_at_cursor(&self) -> String {
        rust_i18n::t!(
            "command_palette.fold_at_cursor",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for folding every block.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_fold_all(), "Fold All");
    /// ```
    #[must_use]
    pub fn command_palette_fold_all(&self) -> String {
        rust_i18n::t!(
            "command_palette.fold_all",
            locale = self.language.to_locale()
        )
        .into_owned()
    }

    /// Returns the command-palette label for unfolding every block.
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let en = Translations::new(Language::English);
    /// assert_eq!(en.command_palette_unfold_all(), "Unfold All");
    /// ```
    #[must_use]
    pub fn command_palette_unfold_all(&self) -> String {
        rust_i18n::t!(
            "command_palette.unfold_all",
            locale = self.language.to_locale()
        )
        .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_language() {
        let translations = Translations::default();
        assert_eq!(translations.language(), Language::English);
    }

    #[test]
    fn test_new_with_language() {
        let translations = Translations::new(Language::French);
        assert_eq!(translations.language(), Language::French);
    }

    #[test]
    fn test_set_language() {
        let mut translations = Translations::new(Language::English);
        translations.set_language(Language::Spanish);
        assert_eq!(translations.language(), Language::Spanish);
    }

    #[test]
    fn test_english_translations() {
        let t = Translations::new(Language::English);
        assert_eq!(t.search_placeholder(), "Search...");
        assert_eq!(t.replace_placeholder(), "Replace...");
        assert_eq!(t.case_sensitive_label(), "Case sensitive");
        assert_eq!(t.previous_match_tooltip(), "Previous match (Shift+F3)");
        assert_eq!(t.next_match_tooltip(), "Next match (F3 / Enter)");
        assert_eq!(t.close_search_tooltip(), "Close search dialog (Esc)");
        assert_eq!(t.replace_current_tooltip(), "Replace current match");
        assert_eq!(t.replace_all_tooltip(), "Replace all matches");
    }

    #[test]
    fn test_french_translations() {
        let t = Translations::new(Language::French);
        assert_eq!(t.search_placeholder(), "Rechercher...");
        assert_eq!(t.replace_placeholder(), "Remplacer...");
        assert_eq!(t.case_sensitive_label(), "Sensible à la casse");
        assert_eq!(t.previous_match_tooltip(), "Résultat précédent (Maj+F3)");
        assert_eq!(t.next_match_tooltip(), "Résultat suivant (F3 / Entrée)");
        assert_eq!(t.close_search_tooltip(), "Fermer la recherche (Échap)");
        assert_eq!(
            t.replace_current_tooltip(),
            "Remplacer l'occurrence actuelle"
        );
        assert_eq!(t.replace_all_tooltip(), "Tout remplacer");
    }

    #[test]
    fn test_spanish_translations() {
        let t = Translations::new(Language::Spanish);
        assert_eq!(t.search_placeholder(), "Buscar...");
        assert_eq!(t.replace_placeholder(), "Reemplazar...");
        assert_eq!(t.case_sensitive_label(), "Distinguir mayúsculas");
        assert_eq!(
            t.previous_match_tooltip(),
            "Coincidencia anterior (Mayús+F3)"
        );
        assert_eq!(
            t.next_match_tooltip(),
            "Siguiente coincidencia (F3 / Enter)"
        );
        assert_eq!(t.close_search_tooltip(), "Cerrar búsqueda (Esc)");
        assert_eq!(
            t.replace_current_tooltip(),
            "Reemplazar coincidencia actual"
        );
        assert_eq!(t.replace_all_tooltip(), "Reemplazar todo");
    }

    #[test]
    fn test_german_translations() {
        let t = Translations::new(Language::German);
        assert_eq!(t.search_placeholder(), "Suchen...");
        assert_eq!(t.replace_placeholder(), "Ersetzen...");
        assert_eq!(t.case_sensitive_label(), "Groß-/Kleinschreibung");
        assert_eq!(
            t.previous_match_tooltip(),
            "Vorheriger Treffer (Umschalt+F3)"
        );
        assert_eq!(t.next_match_tooltip(), "Nächster Treffer (F3 / Enter)");
        assert_eq!(t.close_search_tooltip(), "Suchdialog schließen (Esc)");
        assert_eq!(t.replace_current_tooltip(), "Aktuellen Treffer ersetzen");
        assert_eq!(t.replace_all_tooltip(), "Alle ersetzen");
    }

    #[test]
    fn test_italian_translations() {
        let t = Translations::new(Language::Italian);
        assert_eq!(t.search_placeholder(), "Cerca...");
        assert_eq!(t.replace_placeholder(), "Sostituisci...");
        assert_eq!(t.case_sensitive_label(), "Distingui maiuscole");
        assert_eq!(
            t.previous_match_tooltip(),
            "Risultato precedente (Maiusc+F3)"
        );
        assert_eq!(t.next_match_tooltip(), "Risultato successivo (F3 / Invio)");
        assert_eq!(
            t.close_search_tooltip(),
            "Chiudi finestra di ricerca (Esc)"
        );
        assert_eq!(
            t.replace_current_tooltip(),
            "Sostituisci risultato corrente"
        );
        assert_eq!(t.replace_all_tooltip(), "Sostituisci tutto");
    }

    #[test]
    fn test_portuguese_br_translations() {
        let t = Translations::new(Language::PortugueseBR);
        assert_eq!(t.search_placeholder(), "Pesquisar...");
        assert_eq!(t.replace_placeholder(), "Substituir...");
        assert_eq!(t.case_sensitive_label(), "Diferenciar maiúsculas");
        assert_eq!(
            t.previous_match_tooltip(),
            "Correspondência anterior (Shift+F3)"
        );
        assert_eq!(
            t.next_match_tooltip(),
            "Próxima correspondência (F3 / Enter)"
        );
        assert_eq!(
            t.close_search_tooltip(),
            "Fechar diálogo de pesquisa (Esc)"
        );
        assert_eq!(
            t.replace_current_tooltip(),
            "Substituir correspondência atual"
        );
        assert_eq!(t.replace_all_tooltip(), "Substituir tudo");
    }

    #[test]
    fn test_portuguese_pt_translations() {
        let t = Translations::new(Language::PortuguesePT);
        assert_eq!(t.search_placeholder(), "Pesquisar...");
        assert_eq!(t.replace_placeholder(), "Substituir...");
        assert_eq!(t.case_sensitive_label(), "Diferenciar maiúsculas");
        assert_eq!(
            t.previous_match_tooltip(),
            "Correspondência anterior (Shift+F3)"
        );
        assert_eq!(
            t.next_match_tooltip(),
            "Próxima correspondência (F3 / Enter)"
        );
        assert_eq!(
            t.close_search_tooltip(),
            "Fechar diálogo de pesquisa (Esc)"
        );
        assert_eq!(
            t.replace_current_tooltip(),
            "Substituir correspondência actual"
        );
        assert_eq!(t.replace_all_tooltip(), "Substituir tudo");
    }

    #[test]
    fn test_language_switching() {
        let mut t = Translations::new(Language::English);
        assert_eq!(t.search_placeholder(), "Search...");

        t.set_language(Language::French);
        assert_eq!(t.search_placeholder(), "Rechercher...");

        t.set_language(Language::Spanish);
        assert_eq!(t.search_placeholder(), "Buscar...");

        t.set_language(Language::German);
        assert_eq!(t.search_placeholder(), "Suchen...");

        t.set_language(Language::Italian);
        assert_eq!(t.search_placeholder(), "Cerca...");

        t.set_language(Language::PortugueseBR);
        assert_eq!(t.search_placeholder(), "Pesquisar...");

        t.set_language(Language::PortuguesePT);
        assert_eq!(t.search_placeholder(), "Pesquisar...");

        t.set_language(Language::ChineseSimplified);
        assert_eq!(t.search_placeholder(), "搜索...");
    }

    #[test]
    fn test_context_menu_translations_cover_all_locales() {
        let cases = [
            (
                Language::English,
                [
                    "Undo",
                    "Redo",
                    "Cut",
                    "Copy",
                    "Paste",
                    "Select All",
                    "Reveal in Finder",
                    "Reveal in File Explorer",
                    "Open Containing Folder",
                ],
            ),
            (
                Language::French,
                [
                    "Annuler",
                    "Rétablir",
                    "Couper",
                    "Copier",
                    "Coller",
                    "Tout sélectionner",
                    "Révéler dans le Finder",
                    "Afficher dans l'Explorateur de fichiers",
                    "Ouvrir le dossier contenant",
                ],
            ),
            (
                Language::Spanish,
                [
                    "Deshacer",
                    "Rehacer",
                    "Cortar",
                    "Copiar",
                    "Pegar",
                    "Seleccionar todo",
                    "Mostrar en Finder",
                    "Mostrar en el Explorador de archivos",
                    "Abrir carpeta contenedora",
                ],
            ),
            (
                Language::German,
                [
                    "Rückgängig",
                    "Wiederholen",
                    "Ausschneiden",
                    "Kopieren",
                    "Einfügen",
                    "Alle auswählen",
                    "Im Finder anzeigen",
                    "Im Datei-Explorer anzeigen",
                    "Übergeordneten Ordner öffnen",
                ],
            ),
            (
                Language::Italian,
                [
                    "Annulla azione",
                    "Ripeti",
                    "Taglia",
                    "Copia",
                    "Incolla",
                    "Seleziona tutto",
                    "Visualizza in Finder",
                    "Visualizza in Esplora file",
                    "Apri cartella superiore",
                ],
            ),
            (
                Language::PortugueseBR,
                [
                    "Desfazer",
                    "Refazer",
                    "Recortar",
                    "Copiar",
                    "Colar",
                    "Selecionar Tudo",
                    "Revelar no Finder",
                    "Revelar no Explorador de Arquivos",
                    "Abrir a Pasta Que Contém",
                ],
            ),
            (
                Language::PortuguesePT,
                [
                    "Anular",
                    "Refazer",
                    "Cortar",
                    "Copiar",
                    "Colar",
                    "Selecionar tudo",
                    "Mostrar no Finder",
                    "Mostrar no Explorador de Ficheiros",
                    "Abrir pasta contentora",
                ],
            ),
            (
                Language::ChineseSimplified,
                [
                    "撤消",
                    "恢复",
                    "剪切",
                    "复制",
                    "粘贴",
                    "选择全部",
                    "在访达中显示",
                    "在文件资源管理器中显示",
                    "打开所在的文件夹",
                ],
            ),
        ];

        for (language, expected) in cases {
            let translations = Translations::new(language);
            assert_eq!(translations.context_menu_undo(), expected[0]);
            assert_eq!(translations.context_menu_redo(), expected[1]);
            assert_eq!(translations.context_menu_cut(), expected[2]);
            assert_eq!(translations.context_menu_copy(), expected[3]);
            assert_eq!(translations.context_menu_paste(), expected[4]);
            assert_eq!(translations.context_menu_select_all(), expected[5]);
            assert_eq!(
                translations.context_menu_reveal_in_finder(),
                expected[6]
            );
            assert_eq!(
                translations.context_menu_reveal_in_file_explorer(),
                expected[7]
            );
            assert_eq!(
                translations.context_menu_open_containing_folder(),
                expected[8]
            );
        }
    }

    #[test]
    fn test_command_palette_translations_cover_all_locales() {
        // rust-i18n echoes the key path back when a key is missing from both
        // the locale file and the English fallback, so an untranslated entry
        // shows up as a label starting with "command_palette.".
        let languages = [
            Language::English,
            Language::French,
            Language::Spanish,
            Language::German,
            Language::Italian,
            Language::PortugueseBR,
            Language::PortuguesePT,
            Language::ChineseSimplified,
        ];

        for language in languages {
            let t = Translations::new(language);
            let labels = [
                t.command_palette_placeholder(),
                t.command_palette_no_results(),
                t.command_palette_save(),
                t.command_palette_toggle_comment(),
                t.command_palette_move_line_up(),
                t.command_palette_move_line_down(),
                t.command_palette_duplicate_line_up(),
                t.command_palette_duplicate_line_down(),
                t.command_palette_goto_line(),
                t.command_palette_add_cursor_above(),
                t.command_palette_add_cursor_below(),
                t.command_palette_select_next_occurrence(),
                t.command_palette_toggle_vim_mode(),
                t.command_palette_find(),
                t.command_palette_replace(),
                t.command_palette_fold_at_cursor(),
                t.command_palette_fold_all(),
                t.command_palette_unfold_all(),
            ];

            for label in labels {
                assert!(!label.is_empty(), "empty label for {language:?}");
                assert!(
                    !label.starts_with("command_palette."),
                    "missing translation {label} for {language:?}"
                );
            }
        }
    }

    #[test]
    fn test_command_palette_labels_are_localized() {
        let cases = [
            (Language::English, "Fold All", "Go to Line"),
            (Language::French, "Tout replier", "Aller à la ligne"),
            (Language::Spanish, "Plegar todo", "Ir a la línea"),
            (Language::German, "Alles falten", "Gehe zu Zeile"),
            (Language::Italian, "Comprimi tutto", "Vai alla riga"),
            (Language::PortugueseBR, "Dobrar tudo", "Ir para a linha"),
            (Language::PortuguesePT, "Dobrar tudo", "Ir para a linha"),
            (Language::ChineseSimplified, "全部折叠", "转到行"),
        ];

        for (language, fold_all, goto_line) in cases {
            let t = Translations::new(language);
            assert_eq!(t.command_palette_fold_all(), fold_all);
            assert_eq!(t.command_palette_goto_line(), goto_line);
        }
    }
}
