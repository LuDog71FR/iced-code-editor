//! Integration tests for right-click routing through the real widget tree.
//!
//! The editor's canvas must see the right press that opens the context menu:
//! its `ContextMenuRequested` message runs the caret/selection logic for
//! context clicks and triggers the redraw that paints the menu.

use iced::{Event, Point, mouse};
use iced_code_editor::{CodeEditor, Message};
use iced_test::simulator;

/// Right-clicks `editor`'s view at `point` and returns the published messages.
fn right_click_messages(editor: &CodeEditor, point: Point) -> Vec<Message> {
    let mut ui = simulator(editor.view());
    ui.point_at(point);
    let _ = ui.simulate([
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Right)),
    ]);
    ui.into_messages().collect()
}

#[test]
fn right_click_reaches_the_canvas() {
    let editor = CodeEditor::new("fn main() {\n    let x = 1;\n}", "rs");

    let messages = right_click_messages(&editor, Point::new(120.0, 30.0));

    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::ContextMenuRequested(_))),
        "the canvas never saw the right press: {messages:?}"
    );
}

#[test]
fn right_click_with_a_dialog_open_still_reaches_the_canvas() {
    let mut editor = CodeEditor::new("one\ntwo\nthree", "txt");
    let _ = editor.update(&Message::OpenGotoLine);

    // Below the go-to-line dialog, over the text.
    let messages = right_click_messages(&editor, Point::new(120.0, 300.0));

    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::ContextMenuRequested(_))),
        "a dialog layer must not hide the right press from the canvas: \
         {messages:?}"
    );
}

/// Control: the simulator does route pointer presses to the canvas, so a
/// missing `ContextMenuRequested` above is the editor's fault, not the
/// harness's.
#[test]
fn left_click_reaches_the_canvas() {
    let editor = CodeEditor::new("fn main() {\n    let x = 1;\n}", "rs");

    let mut ui = simulator(editor.view());
    ui.point_at(Point::new(120.0, 30.0));
    let _ = ui.simulate(iced_test::simulator::click());
    let messages: Vec<Message> = ui.into_messages().collect();

    assert!(
        messages
            .iter()
            .any(|message| matches!(message, Message::MouseClick(_))),
        "expected a MouseClick, got {messages:?}"
    );
}
