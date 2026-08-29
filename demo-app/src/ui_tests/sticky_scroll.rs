//! Interface tests for the sticky-scroll header pinned above the canvas.

use super::*;

// ---- Sticky scroll ----

/// A function long enough to scroll past its own header.
fn nested_source() -> String {
    let mut source = String::from("fn outer() {\n");
    for index in 0..80 {
        source.push_str(&format!("    let value_{index} = {index};\n"));
    }
    source.push('}');
    source
}

#[test]
fn test_clicking_the_pinned_header_scrolls_back_to_it() {
    // The whole feature in one gesture: scrolling deep into a block puts a
    // header where there was none, and clicking it navigates. Without the
    // sticky layer the same click lands on the canvas and moves the caret,
    // so the assertion fails loudly rather than silently passing.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&nested_source());

    ui.scroll_code_canvas(10.0);
    assert!(ui.scroll_offset() > 0.0, "the wheel must have moved the viewport");

    let messages = ui.click_top_of_code_area();

    assert!(
        carries(&messages, &EditorMessage::StickyScrollJump(0)),
        "clicking the pinned header must ask the editor to scroll back to it"
    );
    assert_eq!(
        ui.cursor(),
        (0, 0),
        "navigating from a pinned header must leave the caret alone"
    );
}

#[test]
fn test_the_top_row_stays_the_canvas_without_a_pinned_header() {
    // The counterpart: at the top of the file nothing encloses the visible
    // text, so the same click must reach the canvas as it always did.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&nested_source());

    let messages = ui.click_top_of_code_area();

    assert!(
        !carries(&messages, &EditorMessage::StickyScrollJump(0)),
        "nothing may be pinned at the top of the file"
    );
    assert!(
        carries(&messages, &EditorMessage::MouseRelease),
        "with no header pinned the click must reach the canvas"
    );
}

#[test]
fn test_disabling_sticky_scroll_gives_the_top_row_back() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&nested_source());
    ui.toggle_option("Sticky scroll");
    // Fold the panel back: while it is open it covers the code area, and the
    // click below must land on the editor rather than on the options.
    let _ = ui.click("Options ▲");
    ui.scroll_code_canvas(10.0);

    let messages = ui.click_top_of_code_area();

    assert!(
        !carries(&messages, &EditorMessage::StickyScrollJump(0)),
        "a disabled sticky scroll must pin nothing"
    );
    assert!(
        carries(&messages, &EditorMessage::MouseRelease),
        "with sticky scroll off the click must reach the canvas again"
    );
}
