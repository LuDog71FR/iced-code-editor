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
fn drawing(
    editor: &CodeEditor,
    renderer: &iced::Renderer,
    layer: usize,
) -> String {
    use iced::widget::canvas::Program;
    format!(
        "{:?}",
        editor.draw(
            &None,
            renderer,
            &iced::Theme::Dark,
            iced::Rectangle::with_size(iced::Size::new(800.0, 400.0)),
            iced::mouse::Cursor::Unavailable
        )[layer]
    )
}

static FOCUS_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn focused(source: &str) -> CodeEditor {
    let mut editor = CodeEditor::new(source, "rs");
    editor.request_focus();
    editor.has_canvas_focus = true;
    editor.show_cursor = true;
    editor.focus_locked = false;
    editor
}

#[test]
fn editor_config_option_visual_toggles_round_trip() -> Result<(), &'static str>
{
    let _guard = FOCUS_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let renderer = renderer()?;
    for theme in [iced::Theme::Dark, iced::Theme::Light] {
        for (source, layer, setter) in [
            (
                "fn main() {\n    x();\n}",
                0,
                CodeEditor::set_line_numbers_enabled
                    as fn(&mut CodeEditor, bool),
            ),
            ("#ff0000", 0, CodeEditor::set_show_color_previews),
            ("([{}])", 0, CodeEditor::set_bracket_pair_colorization_enabled),
            ("([])", 1, CodeEditor::set_bracket_match_highlight_enabled),
        ] {
            let mut editor = focused(source);
            editor.set_theme(crate::theme::from_iced_theme(&theme));
            let original = drawing(&editor, &renderer, layer);
            setter(&mut editor, false);
            assert_ne!(
                original,
                drawing(&editor, &renderer, layer),
                "{source}"
            );
            setter(&mut editor, true);
            assert_eq!(
                original,
                drawing(&editor, &renderer, layer),
                "{source}"
            );
            assert_eq!(editor.content(), source);
        }
    }
    Ok(())
}

#[test]
fn editor_config_option_folding_restores_hidden_lines() {
    let mut editor =
        CodeEditor::new("fn main() {\n    x();\n    y();\n}", "rs");
    editor.toggle_fold(0);
    let folded = editor.visual_lines_cached(800.0).len();
    editor.set_line_numbers_enabled(false);
    assert!(editor.gutter_width() > 0.0);
    editor.set_folding_enabled(false);
    assert_eq!(editor.visual_lines_cached(800.0).len(), 4);
    editor.set_folding_enabled(true);
    assert_eq!(editor.visual_lines_cached(800.0).len(), folded);
    assert!(folded < 4);
}

#[test]
fn editor_config_option_auto_indent_changes_enter_and_undo() {
    let _guard = FOCUS_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    for (enabled, expected) in [(true, "    x\n    "), (false, "    x\n")] {
        let mut editor = focused("    x");
        editor.cursors.set_single((0, 5));
        editor.set_auto_indent_enabled(enabled);
        let _ = editor.update(&Message::Enter);
        assert_eq!(editor.content(), expected);
        let _ = editor.update(&Message::Undo);
        assert_eq!(editor.content(), "    x");
    }
}

#[test]
fn editor_config_option_auto_brackets_pair_skip_surround_and_undo() {
    let _guard = FOCUS_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    for (open, close) in
        [('(', ')'), ('[', ']'), ('{', '}'), ('"', '"'), ('\'', '\'')]
    {
        for enabled in [true, false] {
            let mut editor = focused("");
            editor.set_auto_close_brackets(enabled);
            let _ = editor.update(&Message::CharacterInput(open));
            assert_eq!(
                editor.content(),
                if enabled {
                    format!("{open}{close}")
                } else {
                    open.to_string()
                }
            );
            let _ = editor.update(&Message::CharacterInput(close));
            assert_eq!(editor.content(), format!("{open}{close}"));
            let mut editor = focused("abc");
            editor.set_auto_close_brackets(enabled);
            editor.cursors.primary_mut().anchor = Some((0, 0));
            editor.cursors.primary_mut().position = (0, 3);
            let _ = editor.update(&Message::CharacterInput(open));
            assert_eq!(
                editor.content(),
                if enabled {
                    format!("{open}abc{close}")
                } else {
                    open.to_string()
                }
            );
            let _ = editor.update(&Message::Undo);
            assert_eq!(editor.content(), "abc");
        }
    }
}

#[test]
fn editor_config_option_vim_changes_navigation_insertion_and_cursor()
-> Result<(), &'static str> {
    let _guard = FOCUS_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let renderer = renderer()?;
    let mut editor = focused("abc");
    let normal = drawing(&editor, &renderer, 1);
    editor.set_vim_enabled(true);
    assert_ne!(normal, drawing(&editor, &renderer, 1));
    let _ = editor.update(&Message::VimKey('l'));
    assert_eq!(editor.cursors.primary_position(), (0, 1));
    let _ = editor.update(&Message::VimKey('i'));
    let _ = editor.update(&Message::CharacterInput('X'));
    assert_eq!(editor.content(), "aXbc");
    editor.set_vim_enabled(false);
    let _ = editor.update(&Message::CharacterInput('Y'));
    assert_eq!(editor.content(), "aXYbc");
    Ok(())
}
