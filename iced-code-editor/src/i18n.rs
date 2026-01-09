//! Internationalization support for the code editor.
//!
//! This module provides translation support for UI text in the search dialog.
//! Currently supports English, French, and Spanish.

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
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{Language, Translations};
    ///
    /// let translations = Translations::new(Language::Spanish);
    /// assert_eq!(translations.language(), Language::Spanish);
    /// ```
    #[must_use]
    pub const fn new(language: Language) -> Self {
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
    pub const fn search_placeholder(&self) -> &'static str {
        match self.language {
            Language::English => "Search...",
            Language::French => "Rechercher...",
            Language::Spanish => "Buscar...",
        }
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
    pub const fn replace_placeholder(&self) -> &'static str {
        match self.language {
            Language::English => "Replace...",
            Language::French => "Remplacer...",
            Language::Spanish => "Reemplazar...",
        }
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
    pub const fn case_sensitive_label(&self) -> &'static str {
        match self.language {
            Language::English => "Case sensitive",
            Language::French => "Sensible à la casse",
            Language::Spanish => "Distinguir mayúsculas",
        }
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
    pub const fn previous_match_tooltip(&self) -> &'static str {
        match self.language {
            Language::English => "Previous match (Shift+F3)",
            Language::French => "Résultat précédent (Maj+F3)",
            Language::Spanish => "Coincidencia anterior (Mayús+F3)",
        }
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
    pub const fn next_match_tooltip(&self) -> &'static str {
        match self.language {
            Language::English => "Next match (F3 / Enter)",
            Language::French => "Résultat suivant (F3 / Entrée)",
            Language::Spanish => "Siguiente coincidencia (F3 / Enter)",
        }
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
    pub const fn close_search_tooltip(&self) -> &'static str {
        match self.language {
            Language::English => "Close search dialog (Esc)",
            Language::French => "Fermer la recherche (Échap)",
            Language::Spanish => "Cerrar búsqueda (Esc)",
        }
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
    pub const fn replace_current_tooltip(&self) -> &'static str {
        match self.language {
            Language::English => "Replace current match",
            Language::French => "Remplacer l'occurrence actuelle",
            Language::Spanish => "Reemplazar coincidencia actual",
        }
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
    pub const fn replace_all_tooltip(&self) -> &'static str {
        match self.language {
            Language::English => "Replace all matches",
            Language::French => "Tout remplacer",
            Language::Spanish => "Reemplazar todo",
        }
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
    fn test_language_switching() {
        let mut t = Translations::new(Language::English);
        assert_eq!(t.search_placeholder(), "Search...");

        t.set_language(Language::French);
        assert_eq!(t.search_placeholder(), "Rechercher...");

        t.set_language(Language::Spanish);
        assert_eq!(t.search_placeholder(), "Buscar...");
    }
}
