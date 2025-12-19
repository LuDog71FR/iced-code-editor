use iced::Color;

/// Theme configuration for the code editor.
///
/// Defines colors for various editor components including background,
/// text, line numbers, gutter, and scrollbar.
#[derive(Debug, Clone)]
pub struct EditorTheme {
    /// Main editor background color
    pub background: Color,
    /// Text content color
    pub text_color: Color,
    /// Line numbers gutter background color
    pub gutter_background: Color,
    /// Border color for the gutter
    pub gutter_border: Color,
    /// Color for line numbers text
    pub line_number_color: Color,
    /// Scrollbar background color
    pub scrollbar_background: Color,
    /// Scrollbar scroller (thumb) color
    pub scroller_color: Color,
}

impl EditorTheme {
    /// Creates a dark theme with VSCode-like colors.
    ///
    /// # Returns
    ///
    /// A dark theme suitable for low-light environments.
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.05, 0.05, 0.07),
            text_color: Color::from_rgb(0.9, 0.9, 0.9),
            gutter_background: Color::from_rgb(0.08, 0.08, 0.10),
            gutter_border: Color::from_rgb(0.15, 0.15, 0.15),
            line_number_color: Color::from_rgb(0.5, 0.5, 0.5),
            scrollbar_background: Color::from_rgb(0.1, 0.1, 0.12),
            scroller_color: Color::from_rgb(0.3, 0.3, 0.35),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_creation() {
        let theme = EditorTheme::dark();

        // Verify very dark background (almost black)
        assert!(theme.background.r < 0.1);
        assert!(theme.background.g < 0.1);
        assert!(theme.background.b < 0.1);

        // Verify bright text for contrast
        assert!(theme.text_color.r > 0.8);
        assert!(theme.text_color.g > 0.8);
        assert!(theme.text_color.b > 0.8);
    }

    #[test]
    fn test_dark_theme_gutter_colors() {
        let theme = EditorTheme::dark();

        // Gutter should be slightly lighter than background
        assert!(theme.gutter_background.r > theme.background.r);
        assert!(theme.gutter_background.g > theme.background.g);
        assert!(theme.gutter_background.b > theme.background.b);

        // Line numbers should be medium gray (readable but not too bright)
        assert!(theme.line_number_color.r > 0.4 && theme.line_number_color.r < 0.6);
    }

    #[test]
    fn test_dark_theme_scrollbar_colors() {
        let theme = EditorTheme::dark();

        // Scrollbar background should be darker than scroller
        assert!(theme.scrollbar_background.r < theme.scroller_color.r);
        assert!(theme.scrollbar_background.g < theme.scroller_color.g);
        assert!(theme.scrollbar_background.b < theme.scroller_color.b);

        // Scroller should be visible (medium gray)
        assert!(theme.scroller_color.r > 0.2);
        assert!(theme.scroller_color.g > 0.2);
        assert!(theme.scroller_color.b > 0.2);
    }

    #[test]
    fn test_theme_clone() {
        let theme1 = EditorTheme::dark();
        let theme2 = theme1.clone();

        // Verify colors are equal
        assert_eq!(theme1.background.r, theme2.background.r);
        assert_eq!(theme1.text_color.r, theme2.text_color.r);
        assert_eq!(theme1.gutter_background.r, theme2.gutter_background.r);
    }
}
