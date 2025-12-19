use iced::Color;

/// The appearance of a code editor.
#[derive(Debug, Clone, Copy)]
pub struct Style {
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
    /// Highlight color for the current line where cursor is located
    pub current_line_highlight: Color,
}

/// The theme catalog of a code editor.
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a code editor.
///
/// This is a shorthand for a function that takes a reference to a
/// [`Theme`](iced::Theme) and returns a [`Style`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for iced::Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(dark)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// Creates a dark theme with VSCode-like colors.
///
/// This is the default styling function. This theme mimics the default VSCode Dark theme with:
/// - Very dark background (#0D0D12)
/// - Light text for contrast
/// - Slightly lighter gutter (#141419)
/// - Medium gray line numbers (#808080)
/// - Subtle scrollbar colors
/// - Dark bluish current line highlight
pub fn dark(_theme: &iced::Theme) -> Style {
    Style {
        background: Color::from_rgb(0.05, 0.05, 0.07), // #0D0D12
        text_color: Color::from_rgb(0.9, 0.9, 0.9),    // #E6E6E6
        gutter_background: Color::from_rgb(0.08, 0.08, 0.10), // #141419
        gutter_border: Color::from_rgb(0.15, 0.15, 0.15), // #262626
        line_number_color: Color::from_rgb(0.5, 0.5, 0.5), // #808080
        scrollbar_background: Color::from_rgb(0.1, 0.1, 0.12), // #1A1A1F
        scroller_color: Color::from_rgb(0.3, 0.3, 0.35), // #4D4D59
        current_line_highlight: Color::from_rgb(0.15, 0.15, 0.2), // #262633
    }
}

/// Creates a light theme with VSCode-like colors.
///
/// This theme mimics the default VSCode Light theme with:
/// - White background (#FFFFFF)
/// - Dark text for contrast
/// - Light gray gutter (#F3F3F3)
/// - Medium gray line numbers (#858585)
/// - Subtle scrollbar colors
/// - Light grayish-blue current line highlight for cursor visibility
pub fn light(_theme: &iced::Theme) -> Style {
    Style {
        background: Color::from_rgb(1.0, 1.0, 1.0), // #FFFFFF
        text_color: Color::from_rgb(0.0, 0.0, 0.0), // #000000
        gutter_background: Color::from_rgb(0.953, 0.953, 0.953), // #F3F3F3
        gutter_border: Color::from_rgb(0.9, 0.9, 0.9), // #E5E5E5
        line_number_color: Color::from_rgb(0.522, 0.522, 0.522), // #858585
        scrollbar_background: Color::from_rgb(0.961, 0.961, 0.961), // #F5F5F5
        scroller_color: Color::from_rgb(0.784, 0.784, 0.784), // #C8C8C8
        current_line_highlight: Color::from_rgb(0.95, 0.95, 0.97), // #F2F2F7
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme_creation() {
        let theme = iced::Theme::Dark;
        let style = dark(&theme);

        // Verify very dark background (almost black)
        assert!(style.background.r < 0.1);
        assert!(style.background.g < 0.1);
        assert!(style.background.b < 0.1);

        // Verify bright text for contrast
        assert!(style.text_color.r > 0.8);
        assert!(style.text_color.g > 0.8);
        assert!(style.text_color.b > 0.8);
    }

    #[test]
    fn test_dark_theme_gutter_colors() {
        let theme = iced::Theme::Dark;
        let style = dark(&theme);

        // Gutter should be slightly lighter than background
        assert!(style.gutter_background.r > style.background.r);
        assert!(style.gutter_background.g > style.background.g);
        assert!(style.gutter_background.b > style.background.b);

        // Line numbers should be medium gray (readable but not bright)
        let line_num_r = style.line_number_color.r;
        assert!(line_num_r > 0.4 && line_num_r < 0.6);
    }

    #[test]
    fn test_dark_theme_scrollbar_colors() {
        let theme = iced::Theme::Dark;
        let style = dark(&theme);

        // Scrollbar background should be darker than scroller
        assert!(style.scrollbar_background.r < style.scroller_color.r);
        assert!(style.scrollbar_background.g < style.scroller_color.g);
        assert!(style.scrollbar_background.b < style.scroller_color.b);

        // Scroller should be visible (medium gray)
        assert!(style.scroller_color.r > 0.2);
        assert!(style.scroller_color.g > 0.2);
        assert!(style.scroller_color.b > 0.2);
    }

    #[test]
    fn test_style_copy() {
        let theme = iced::Theme::Dark;
        let style1 = dark(&theme);
        let style2 = style1;

        // Verify colors are approximately equal (using epsilon for float comparison)
        assert!(
            (style1.background.r - style2.background.r).abs() < f32::EPSILON
        );
        assert!(
            (style1.text_color.r - style2.text_color.r).abs() < f32::EPSILON
        );
        assert!(
            (style1.gutter_background.r - style2.gutter_background.r).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn test_catalog_default() {
        let theme = iced::Theme::Dark;
        let class = <iced::Theme as Catalog>::default();
        let style = theme.style(&class);

        // Should produce the dark theme by default
        assert!(style.background.r < 0.1);
        assert!(style.text_color.r > 0.8);
    }

    #[test]
    fn test_light_theme_creation() {
        let theme = iced::Theme::Light;
        let style = light(&theme);

        // Verify bright background (white)
        assert!(style.background.r > 0.9);
        assert!(style.background.g > 0.9);
        assert!(style.background.b > 0.9);

        // Verify dark text for contrast
        assert!(style.text_color.r < 0.2);
        assert!(style.text_color.g < 0.2);
        assert!(style.text_color.b < 0.2);
    }

    #[test]
    fn test_light_theme_gutter_colors() {
        let theme = iced::Theme::Light;
        let style = light(&theme);

        // Gutter should be slightly darker than background
        assert!(style.gutter_background.r < style.background.r);
        assert!(style.gutter_background.g < style.background.g);
        assert!(style.gutter_background.b < style.background.b);

        // Gutter should still be very light (light gray)
        assert!(style.gutter_background.r > 0.9);

        // Line numbers should be medium gray (readable)
        let line_num_r = style.line_number_color.r;
        assert!(line_num_r > 0.4 && line_num_r < 0.6);
    }

    #[test]
    fn test_light_theme_scrollbar_colors() {
        let theme = iced::Theme::Light;
        let style = light(&theme);

        // Scrollbar background should be lighter than scroller
        assert!(style.scrollbar_background.r > style.scroller_color.r);
        assert!(style.scrollbar_background.g > style.scroller_color.g);
        assert!(style.scrollbar_background.b > style.scroller_color.b);

        // Scroller should be visible (medium-light gray)
        assert!(style.scroller_color.r > 0.7);
        assert!(style.scroller_color.g > 0.7);
        assert!(style.scroller_color.b > 0.7);
    }

    #[test]
    fn test_light_vs_dark_contrast() {
        let light_style = light(&iced::Theme::Light);
        let dark_style = dark(&iced::Theme::Dark);

        // Light theme should have brighter background than dark theme
        assert!(light_style.background.r > dark_style.background.r);

        // Light theme should have darker text than dark theme
        assert!(light_style.text_color.r < dark_style.text_color.r);
    }
}
