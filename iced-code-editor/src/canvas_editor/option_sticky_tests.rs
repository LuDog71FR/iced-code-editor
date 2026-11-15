use super::*;
#[test]
fn editor_config_option_sticky_layer_tracks_scroll_toggle_and_folding() {
    let mut editor =
        CodeEditor::new("fn main() {\n    x();\n    y();\n}", "rs");
    let lines = editor.visual_lines_cached(800.0);
    assert!(editor.create_sticky_scroll_layer(&lines).is_none());
    editor.viewport_scroll = editor.line_height();
    assert!(editor.create_sticky_scroll_layer(&lines).is_some());
    editor.set_sticky_scroll_enabled(false);
    assert!(editor.create_sticky_scroll_layer(&lines).is_none());
    editor.set_folding_enabled(false);
    editor.set_sticky_scroll_enabled(true);
    let lines = editor.visual_lines_cached(800.0);
    assert!(editor.create_sticky_scroll_layer(&lines).is_some());
}
