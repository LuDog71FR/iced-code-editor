use iced::Color;

pub struct EditorTheme {
    pub background: Color,
    pub text_color: Color,
    pub gutter_background: Color,
    pub gutter_border: Color,
    pub line_number_color: Color,
}

impl EditorTheme {
    pub fn dark() -> Self {
        Self {
            background: Color::from_rgb(0.12, 0.12, 0.12), // #1e1e1e
            gutter_background: Color::from_rgb(0.15, 0.15, 0.15), // #252525
            line_number_color: Color::from_rgb(0.52, 0.52, 0.52), // #858585
            gutter_border: Color::from_rgb(0.2, 0.2, 0.2),
            text_color: Color::from_rgb(0.85, 0.85, 0.85),
        }
    }
}
