use super::*;
use iced::advanced::renderer::Headless;

fn renderer() -> Result<iced::Renderer, &'static str> {
    iced::futures::executor::block_on(iced::Renderer::new(
        iced::Font::DEFAULT,
        iced::Pixels(13.0),
        Some("tiny-skia"),
    ))
    .ok_or("software renderer unavailable")
}

// Record the actual software canvas commands, including paths, glyphs and colors.
fn drawing(editor: &CodeEditor, renderer: &iced::Renderer) -> String {
    use iced::widget::canvas::Program;
    format!(
        "{:?}",
        editor.draw(
            &None,
            renderer,
            &iced::Theme::Dark,
            iced::Rectangle::with_size(iced::Size::new(800.0, 400.0)),
            iced::mouse::Cursor::Unavailable
        )[0]
    )
}

#[test]
fn editor_config_indent_style_invalidates_warm_geometry()
-> Result<(), &'static str> {
    let renderer = renderer()?;
    let mut editor = CodeEditor::new("        x\n\t x\n\n        y", "txt");
    let original = drawing(&editor, &renderer);
    editor.set_indent_style(IndentStyle::Spaces(2));
    let changed = drawing(&editor, &renderer);
    assert_ne!(original, changed, "guide spacing must change immediately");
    editor.set_indent_style(IndentStyle::Spaces(4));
    assert_eq!(original, drawing(&editor, &renderer));
    Ok(())
}

#[test]
fn editor_config_indent_toggles_change_real_commands_without_editing()
-> Result<(), &'static str> {
    let renderer = renderer()?;
    let source = "        x\n\t x\n\n        y";
    let mut editor = CodeEditor::new(source, "txt");
    editor.set_show_whitespace(false);
    let guides = drawing(&editor, &renderer);
    editor.set_show_indent_guides(false);
    let plain = drawing(&editor, &renderer);
    assert_ne!(guides, plain);
    editor.set_show_whitespace(true);
    let whitespace = drawing(&editor, &renderer);
    assert_ne!(plain, whitespace);
    assert!(whitespace.contains('·'));
    assert!(whitespace.contains('→'));
    editor.set_show_whitespace(false);
    assert_eq!(plain, drawing(&editor, &renderer));
    editor.set_show_indent_guides(true);
    assert_eq!(guides, drawing(&editor, &renderer));
    assert_eq!(editor.content(), source);
    Ok(())
}

#[test]
fn editor_config_indent_tab_messages_follow_style_and_undo() {
    for (style, expected) in [
        (IndentStyle::Spaces(2), "  x"),
        (IndentStyle::Spaces(4), "    x"),
        (IndentStyle::Tab, "\tx"),
    ] {
        let mut editor = CodeEditor::new("x", "txt");
        editor.set_indent_style(style);
        let _ = editor.update(&Message::Tab);
        assert_eq!(editor.content(), expected);
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.content(), "x");
    }
}

#[test]
fn editor_config_indent_levels_cover_mixed_and_blank_lines() {
    use super::features::indent_guides::guide_levels;
    let editor = CodeEditor::new("    x\n\n\t  y\n  z", "txt");
    for (unit, levels) in [(2, [2, 2, 3, 1]), (4, [1, 1, 1, 0])] {
        for (line, expected) in levels.into_iter().enumerate() {
            assert_eq!(guide_levels(&editor.buffer, line, unit), expected);
        }
    }
}

#[test]
fn editor_config_indent_rendering_survives_wrap_fold_scroll_and_themes()
-> Result<(), &'static str> {
    let renderer = renderer()?;
    for theme in [iced::Theme::Dark, iced::Theme::Light] {
        let mut editor = CodeEditor::new(
            "fn main() {\n        abcdefghijkl();\n\n        y();\n}",
            "rs",
        )
        .with_wrap_column(Some(12));
        editor.set_theme(crate::theme::from_iced_theme(&theme));
        assert_ne!(editor.style.indent_guide_color, editor.style.background);
        assert_ne!(editor.style.whitespace_color, editor.style.background);
        let original = drawing(&editor, &renderer);
        let style = editor.style;
        let mut recolored = style;
        recolored.indent_guide_color = iced::Color::from_rgb(1.0, 0.0, 0.0);
        editor.set_theme(recolored);
        assert_ne!(original, drawing(&editor, &renderer));
        editor.set_theme(style);
        let lines = editor.visual_lines_cached(800.0);
        assert!(lines.iter().any(|v| !v.is_first_segment()));
        editor.toggle_fold(0);
        assert_ne!(original, drawing(&editor, &renderer));
        editor.toggle_fold(0);
        assert_eq!(original, drawing(&editor, &renderer));
        editor.set_wrap_enabled(false);
        editor.horizontal_scroll_offset = 7.0;
        editor.viewport_scroll = editor.line_height;
        editor.content_cache.clear();
        let scrolled = drawing(&editor, &renderer);
        editor.set_show_indent_guides(false);
        assert_ne!(scrolled, drawing(&editor, &renderer));
        assert_eq!(
            editor.content(),
            "fn main() {\n        abcdefghijkl();\n\n        y();\n}"
        );
    }
    Ok(())
}
