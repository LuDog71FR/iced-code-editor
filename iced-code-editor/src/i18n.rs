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
}

/// Generates the [`Translations`] accessors from a table of
/// name / key / summary / English wording.
///
/// Every accessor has the same body — look the key up in the active locale and
/// own the result — so writing them out cost about eighteen lines each, of
/// which the only information was a name-to-key mapping. Worse, the repetition
/// hid the one mistake that matters here: a misspelled key still compiles and
/// only surfaces at runtime, as the raw key path on screen.
///
/// The generated `# Examples` block asserts the English wording, so a key that
/// stops resolving fails a doctest instead of reaching a user. The other seven
/// locales are covered by the unit tests — structurally by
/// `test_every_locale_defines_exactly_the_keys_english_defines`, by wording
/// through `test_french_translations` and its siblings — which is the right
/// split, since a doctest is a usage example first.
macro_rules! translations {
    ($($name:ident => $key:literal, $summary:literal, $english:literal;)*) => {
        impl Translations {
            $(
                #[doc = $summary]
                #[doc = ""]
                #[doc = "# Examples"]
                #[doc = ""]
                #[doc = "```"]
                #[doc = "use iced_code_editor::{Language, Translations};"]
                #[doc = ""]
                #[doc = concat!(
                    "assert_eq!(Translations::new(Language::English).",
                    stringify!($name),
                    "(), ",
                    stringify!($english),
                    ");"
                )]
                #[doc = "```"]
                #[must_use]
                pub fn $name(&self) -> String {
                    rust_i18n::t!($key, locale = self.language.to_locale())
                        .into_owned()
                }
            )*
        }
    };
}

// The generated accessors: one row per translated string. The hand-written
// members of `Translations` live in the `impl` above; this is everything whose
// body is nothing but a lookup.
translations! {
    search_placeholder => "search.placeholder",
        "Returns the placeholder text for the search input field.",
        "Search...";
    replace_placeholder => "replace.placeholder",
        "Returns the placeholder text for the replace input field.",
        "Replace...";
    case_sensitive_label => "settings.case_sensitive_label",
        "Returns the label text for the case sensitive checkbox.",
        "Case sensitive";
    previous_match_tooltip => "search.previous_match_tooltip",
        "Returns the tooltip text for the previous match button.",
        "Previous match (Shift+F3)";
    next_match_tooltip => "search.next_match_tooltip",
        "Returns the tooltip text for the next match button.",
        "Next match (F3 / Enter)";
    close_search_tooltip => "search.close_tooltip",
        "Returns the tooltip text for the close search dialog button.",
        "Close search dialog (Esc)";
    replace_current_tooltip => "replace.current_tooltip",
        "Returns the tooltip text for the replace current match button.",
        "Replace current match";
    replace_all_tooltip => "replace.all_tooltip",
        "Returns the tooltip text for the replace all matches button.",
        "Replace all matches";
    context_menu_undo => "context_menu.undo",
        "Returns the context-menu label for undo.",
        "Undo";
    context_menu_redo => "context_menu.redo",
        "Returns the context-menu label for redo.",
        "Redo";
    context_menu_cut => "context_menu.cut",
        "Returns the context-menu label for cut.",
        "Cut";
    context_menu_copy => "context_menu.copy",
        "Returns the context-menu label for copy.",
        "Copy";
    context_menu_paste => "context_menu.paste",
        "Returns the context-menu label for paste.",
        "Paste";
    context_menu_select_all => "context_menu.select_all",
        "Returns the context-menu label for select all.",
        "Select All";
    context_menu_reveal_in_finder => "context_menu.reveal_in_finder",
        "Returns the macOS context-menu label for revealing a file.",
        "Reveal in Finder";
    context_menu_reveal_in_file_explorer => "context_menu.reveal_in_file_explorer",
        "Returns the Windows context-menu label for revealing a file.",
        "Reveal in File Explorer";
    context_menu_open_containing_folder => "context_menu.open_containing_folder",
        "Returns the Linux context-menu label for opening a file's parent folder.",
        "Open Containing Folder";
    command_palette_placeholder => "command_palette.placeholder",
        "Returns the command-palette label for the filter input's placeholder.",
        "Type a command...";
    command_palette_no_results => "command_palette.no_results",
        "Returns the command-palette label for the empty-result message.",
        "No matching command";
    command_palette_save => "command_palette.save",
        "Returns the command-palette label for saving the document.",
        "Save";
    command_palette_toggle_comment => "command_palette.toggle_comment",
        "Returns the command-palette label for toggling the line comment.",
        "Toggle Line Comment";
    command_palette_move_line_up => "command_palette.move_line_up",
        "Returns the command-palette label for moving the current line up.",
        "Move Line Up";
    command_palette_move_line_down => "command_palette.move_line_down",
        "Returns the command-palette label for moving the current line down.",
        "Move Line Down";
    command_palette_duplicate_line_up => "command_palette.duplicate_line_up",
        "Returns the command-palette label for duplicating the current line above.",
        "Duplicate Line Up";
    command_palette_duplicate_line_down => "command_palette.duplicate_line_down",
        "Returns the command-palette label for duplicating the current line below.",
        "Duplicate Line Down";
    command_palette_goto_line => "command_palette.goto_line",
        "Returns the command-palette label for jumping to a line.",
        "Go to Line";
    command_palette_add_cursor_above => "command_palette.add_cursor_above",
        "Returns the command-palette label for adding a cursor on the line above.",
        "Add Cursor Above";
    command_palette_add_cursor_below => "command_palette.add_cursor_below",
        "Returns the command-palette label for adding a cursor on the line below.",
        "Add Cursor Below";
    command_palette_select_next_occurrence => "command_palette.select_next_occurrence",
        "Returns the command-palette label for selecting the next occurrence.",
        "Select Next Occurrence";
    command_palette_toggle_vim_mode => "command_palette.toggle_vim_mode",
        "Returns the command-palette label for toggling Vim mode.",
        "Toggle Vim Mode";
    command_palette_find => "command_palette.find",
        "Returns the command-palette label for opening the search dialog.",
        "Find";
    command_palette_replace => "command_palette.replace",
        "Returns the command-palette label for opening the search-and-replace dialog.",
        "Replace";
    command_palette_fold_at_cursor => "command_palette.fold_at_cursor",
        "Returns the command-palette label for folding the block at the cursor.",
        "Toggle Fold at Cursor";
    command_palette_fold_all => "command_palette.fold_all",
        "Returns the command-palette label for folding every block.",
        "Fold All";
    command_palette_unfold_all => "command_palette.unfold_all",
        "Returns the command-palette label for unfolding every block.",
        "Unfold All";
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// Every locale file, paired with the name used in failure messages.
    ///
    /// English comes first: it is the reference every other file is compared
    /// against, and the fallback `rust_i18n` resolves a missing key through.
    const LOCALE_FILES: [(&str, &str); 8] = [
        ("en", include_str!("../locales/en.yml")),
        ("fr", include_str!("../locales/fr.yml")),
        ("es", include_str!("../locales/es.yml")),
        ("de", include_str!("../locales/de.yml")),
        ("it", include_str!("../locales/it.yml")),
        ("pt-BR", include_str!("../locales/pt-BR.yml")),
        ("pt-PT", include_str!("../locales/pt-PT.yml")),
        ("zh-CN", include_str!("../locales/zh-CN.yml")),
    ];

    /// Collects the `section.key` paths a locale file defines.
    ///
    /// Deliberately a five-line reader rather than a YAML dependency: the
    /// files are a flat list of `section:` headers with two-space-indented
    /// entries under them, and reading them structurally is the whole point —
    /// going through `rust_i18n` is exactly what cannot see a missing key.
    fn locale_keys(yaml: &str) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        let mut section = String::new();

        for line in yaml.lines() {
            let line = line.trim_end();
            if line.trim_start().is_empty()
                || line.trim_start().starts_with('#')
            {
                continue;
            }

            match line.strip_prefix("  ") {
                // Indented: an entry belonging to the current section.
                Some(entry) => {
                    if let Some((name, _)) = entry.split_once(':') {
                        keys.insert(format!("{section}.{}", name.trim()));
                    }
                }
                // Flush left: a new section header.
                None => {
                    if let Some((name, _)) = line.split_once(':') {
                        section = name.trim().to_string();
                    }
                }
            }
        }

        keys
    }

    #[test]
    fn test_locale_keys_reads_the_section_and_entry_layout() {
        // Guards the guard: a reader that silently returned nothing would make
        // every comparison below pass on an empty set.
        let keys = locale_keys(
            "# a comment\nsearch:\n  placeholder: \"Search...\"\n\nreplace:\n  all_tooltip: \"Replace all matches\"\n",
        );

        assert_eq!(
            keys.iter().map(String::as_str).collect::<Vec<_>>(),
            ["replace.all_tooltip", "search.placeholder"]
        );
    }

    #[test]
    fn test_every_locale_defines_exactly_the_keys_english_defines() {
        // The structural check the runtime ones cannot perform: with
        // `fallback = "en"`, a key present in `en.yml` and missing from
        // `fr.yml` resolves to the *English* string, so no assertion made
        // through `Translations` can tell the two apart. Comparing the files
        // themselves is what catches a translator skipping an entry.
        let (_, english_source) = LOCALE_FILES[0];
        let english = locale_keys(english_source);
        assert!(
            english.contains("command_palette.fold_all"),
            "en.yml did not parse as expected"
        );

        for (name, source) in LOCALE_FILES {
            let keys = locale_keys(source);
            let missing: Vec<&String> = english.difference(&keys).collect();
            let extra: Vec<&String> = keys.difference(&english).collect();

            assert!(missing.is_empty(), "{name}.yml is missing {missing:?}");
            assert!(
                extra.is_empty(),
                "{name}.yml defines {extra:?}, which en.yml does not"
            );
        }
    }

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
    fn test_every_command_palette_accessor_resolves_to_a_defined_key() {
        // What this can catch: a typo in an accessor's `t!` key path. rust-i18n
        // echoes the path back when it resolves to nothing, so the label comes
        // out starting with "command_palette.".
        //
        // What it cannot: a key missing from one locale. `fallback = "en"`
        // makes that resolve to the English string, which is neither empty nor
        // a key path. Locale completeness is checked structurally instead, by
        // `test_every_locale_defines_exactly_the_keys_english_defines`; the
        // loop over all eight languages here is what proves every accessor is
        // reachable in each of them, not that each has its own wording.
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
