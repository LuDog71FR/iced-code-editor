use super::*;

#[test]
fn editor_config_wrap_zero_normalizes_to_one_column() {
    let editor = CodeEditor::new("abcd", "txt").with_wrap_column(Some(0));
    assert_eq!(editor.wrap_column, Some(1));
    assert_eq!(editor.visual_lines_cached(800.0).len(), 4);
}

#[test]
fn editor_config_wrap_toggle_preserves_text_and_coordinates()
-> Result<(), &'static str> {
    let source = "ab\t中文abcdefgh\n\nlast";
    let mut editor = CodeEditor::new(source, "txt").with_wrap_column(Some(4));
    editor.viewport_width = 800.0;
    for enabled in [true, false, true, false, true] {
        editor.set_wrap_enabled(enabled);
        let lines = editor.visual_lines_cached(800.0);
        assert_eq!(lines.iter().filter(|v| v.logical_line == 1).count(), 1);
        assert_eq!(lines.len() > 3, enabled);
        for col in [0, 1, 2, 3, 4, 6, 10] {
            let point = editor
                .point_from_position(0, col)
                .ok_or("visible logical position missing")?;
            assert_eq!(
                editor.calculate_cursor_from_point(point),
                Some((0, col))
            );
        }
        assert_eq!(editor.content().as_bytes(), source.as_bytes());
    }
    Ok(())
}

#[test]
fn editor_config_wrap_builder_invalidates_warm_drawing()
-> Result<(), &'static str> {
    use iced::advanced::renderer::Headless;
    use iced::widget::canvas::Program;
    let renderer = iced::futures::executor::block_on(iced::Renderer::new(
        iced::Font::DEFAULT,
        iced::Pixels(13.0),
        Some("tiny-skia"),
    ))
    .ok_or("software renderer unavailable")?;
    let editor = CodeEditor::new("ab\t中文abcdefghijkl", "txt");
    let bounds = iced::Rectangle::with_size(iced::Size::new(800.0, 400.0));
    assert!(
        !editor
            .draw(
                &None,
                &renderer,
                &iced::Theme::Dark,
                bounds,
                iced::mouse::Cursor::Unavailable
            )
            .is_empty()
    );
    let mut editor = editor.with_wrap_column(Some(4));
    let rebuilt = Cell::new(false);
    let _ = editor
        .content_cache
        .draw(&renderer, bounds.size(), |_| rebuilt.set(true));
    assert!(
        rebuilt.get(),
        "column change must invalidate warmed text geometry"
    );
    for theme in [iced::Theme::Dark, iced::Theme::Light] {
        editor.set_wrap_column(Some(8));
        assert!(
            !editor
                .draw(
                    &None,
                    &renderer,
                    &theme,
                    bounds,
                    iced::mouse::Cursor::Unavailable
                )
                .is_empty()
        );
        editor.set_wrap_column(Some(4));
        for cache in [&editor.content_cache, &editor.overlay_cache] {
            let rebuilt = Cell::new(false);
            let _ = cache.draw(&renderer, bounds.size(), |_| rebuilt.set(true));
            assert!(
                rebuilt.get(),
                "runtime column change must invalidate geometry"
            );
        }
    }
    Ok(())
}

#[test]
fn editor_config_wrap_runtime_width_resize_selection_and_undo()
-> Result<(), &'static str> {
    let source = "ab\t中文abcdefghijkl\n\nlast";
    let mut editor = CodeEditor::new(source, "txt");
    editor.viewport_width = 800.0;
    let _ = editor.visual_lines_cached(800.0);
    for width in [Some(4), Some(8), Some(0), None] {
        editor.set_wrap_column(width);
        assert_eq!(editor.wrap_column(), width.map(|n| n.max(1)));
        let wide = editor.visual_lines_cached(800.0);
        let narrow = editor.visual_lines_cached(
            editor.gutter_width() + editor.char_width * 3.0,
        );
        if width.is_none() {
            assert!(narrow.len() > wide.len());
        }
        let start = editor
            .point_from_position(0, 1)
            .ok_or("visible logical position missing")?;
        let end = editor
            .point_from_position(0, 8)
            .ok_or("visible logical position missing")?;
        let _ = editor.update(&Message::MouseClick(start));
        let _ = editor.update(&Message::MouseDrag(end));
        let _ = editor.update(&Message::MouseRelease);
        assert_eq!(editor.selected_text().as_deref(), Some("b\t中文abc"));
        assert_eq!(editor.content(), source);
    }
    let _ = editor.update(&Message::CharacterInput('x'));
    editor.set_wrap_column(Some(3));
    let _ = editor.update(&Message::Undo);
    assert_eq!(editor.content(), source);
    editor.set_wrap_enabled(false);
    editor.horizontal_scroll_offset = 42.0;
    editor.set_wrap_column(Some(7));
    assert_eq!(editor.horizontal_scroll_offset, 42.0);
    editor.set_wrap_enabled(true);
    assert_eq!(editor.horizontal_scroll_offset, 0.0);
    Ok(())
}

#[test]
fn editor_config_wrap_folds_and_font_resize_keep_layout_consistent()
-> Result<(), &'static str> {
    let mut editor =
        CodeEditor::new("fn main() {\n    abcdefghijkl();\n}\n", "rs")
            .with_wrap_column(Some(6));
    editor.viewport_width = 800.0;
    let before = editor.visual_lines_cached(800.0);
    editor.toggle_fold(0);
    let folded = editor.visual_lines_cached(800.0);
    assert!(folded.len() < before.len());
    assert!(folded.iter().all(|v| v.logical_line != 1));
    editor.set_wrap_enabled(false);
    assert!(
        editor.visual_lines_cached(800.0).iter().all(|v| v.segment_index == 0)
    );
    editor.toggle_fold(0);
    editor.set_wrap_enabled(true);
    assert_eq!(*editor.visual_lines_cached(800.0), *before);
    editor.set_font_size(20.0, true);
    let p = editor
        .point_from_position(1, 8)
        .ok_or("visible logical position missing")?;
    assert_eq!(editor.calculate_cursor_from_point(p), Some((1, 8)));
    assert!(p.y >= editor.line_height());
    Ok(())
}
