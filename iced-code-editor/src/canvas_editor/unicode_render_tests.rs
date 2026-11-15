use super::*;
use iced::advanced::graphics::geometry::Renderer as _;
use iced::advanced::renderer::Headless;
use iced::widget::canvas::Program;

fn pixels(
    source: &str,
    language: &str,
    whitespace: bool,
    theme: &iced::Theme,
) -> Result<Vec<u8>, &'static str> {
    let mut renderer = iced::futures::executor::block_on(iced::Renderer::new(
        iced::Font::DEFAULT,
        14.0.into(),
        Some("tiny-skia"),
    ))
    .ok_or("renderer unavailable")?;
    let mut editor = CodeEditor::new(source, language);
    editor.set_theme(crate::theme::from_iced_theme(theme));
    editor.set_line_numbers_enabled(false);
    editor.set_folding_enabled(false);
    editor.set_show_whitespace(whitespace);
    editor.set_show_indent_guides(false);
    let bounds = iced::Rectangle::with_size(iced::Size::new(320.0, 100.0));
    for geometry in editor.draw(
        &None,
        &renderer,
        theme,
        bounds,
        iced::mouse::Cursor::Unavailable,
    ) {
        renderer.draw_geometry(geometry);
    }
    assert_eq!(editor.content(), source);
    Ok(renderer.screenshot(iced::Size::new(320, 100), 1.0, iced::Color::BLACK))
}

#[test]
fn editor_config_unicode_draws_cjk_glyphs_instead_of_blank_cells()
-> Result<(), &'static str> {
    for theme in [iced::Theme::Light, iced::Theme::Dark] {
        for language in ["txt", "rust"] {
            for whitespace in [false, true] {
                let draw = |s| pixels(s, language, whitespace, &theme);
                // Equal-width CJK have distinct glyphs; a background/whitespace-only
                // rendering would produce identical pixels for these two strings.
                assert!(
                    draw("AB")? != draw("CD")?,
                    "ASCII control must draw glyphs"
                );
                assert!(
                    draw("中文")? != draw("测试")?,
                    "CJK must draw distinct glyph pixels"
                );
            }
        }
    }
    Ok(())
}
