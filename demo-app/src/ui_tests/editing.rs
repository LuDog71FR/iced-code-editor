//! Interface tests for editing the buffer from the keyboard: moving and
//! duplicating lines, undo/redo, the clipboard, the focus gate, typing,
//! auto-indentation, comment toggling and auto-closing brackets.

use super::*;

// ---- Move line ----

#[test]
fn test_alt_down_moves_the_current_line_down() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta\ngamma");

    let messages = ui.press(Key::Named(Named::ArrowDown), Modifiers::ALT);

    assert!(carries(&messages, &EditorMessage::MoveLineDown));
    assert_eq!(ui.content(), "beta\nalpha\ngamma");
}

#[test]
fn test_alt_up_moves_the_current_line_up() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta\ngamma");
    // Step onto the second line, so there is a line above to swap with.
    let _ = ui.tap(Named::ArrowDown);

    let messages = ui.press(Key::Named(Named::ArrowUp), Modifiers::ALT);

    assert!(carries(&messages, &EditorMessage::MoveLineUp));
    assert_eq!(ui.content(), "beta\nalpha\ngamma");
}

#[test]
fn test_alt_up_on_the_first_line_changes_nothing() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");

    let _ = ui.press(Key::Named(Named::ArrowUp), Modifiers::ALT);

    assert_eq!(ui.content(), "alpha\nbeta");
}

// ---- Duplicate line ----

#[test]
fn test_shift_alt_down_duplicates_the_current_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");

    let messages = ui
        .press(Key::Named(Named::ArrowDown), Modifiers::ALT | Modifiers::SHIFT);

    assert!(carries(&messages, &EditorMessage::DuplicateLineDown));
    assert_eq!(ui.content(), "alpha\nalpha\nbeta");
}

#[test]
fn test_shift_alt_up_duplicates_the_current_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");

    let messages =
        ui.press(Key::Named(Named::ArrowUp), Modifiers::ALT | Modifiers::SHIFT);

    assert!(carries(&messages, &EditorMessage::DuplicateLineUp));
    assert_eq!(ui.content(), "alpha\nalpha\nbeta");
}

// ---- Undo / redo ----

#[test]
fn test_ctrl_z_undoes_an_edit_made_from_the_keyboard() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha");
    let _ = ui
        .press(Key::Named(Named::ArrowDown), Modifiers::ALT | Modifiers::SHIFT);
    assert_eq!(ui.content(), "alpha\nalpha");

    let messages = ui.press(Key::Character("z".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::Undo));
    assert_eq!(ui.content(), "alpha");
}

#[test]
fn test_ctrl_y_redoes_the_undone_edit() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha");
    let _ = ui
        .press(Key::Named(Named::ArrowDown), Modifiers::ALT | Modifiers::SHIFT);
    let _ = ui.press(Key::Character("z".into()), Modifiers::COMMAND);
    assert_eq!(ui.content(), "alpha");

    let messages = ui.press(Key::Character("y".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::Redo));
    assert_eq!(ui.content(), "alpha\nalpha");
}

#[test]
fn test_ctrl_shift_z_also_redoes() {
    // The second redo binding, which shares its key with undo and is told
    // apart by Shift alone.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha");
    let _ = ui
        .press(Key::Named(Named::ArrowDown), Modifiers::ALT | Modifiers::SHIFT);
    let _ = ui.press(Key::Character("z".into()), Modifiers::COMMAND);

    let messages = ui.press(
        Key::Character("z".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    assert!(carries(&messages, &EditorMessage::Redo));
    assert_eq!(ui.content(), "alpha\nalpha");
}

// ---- Page Up / Page Down ----

/// Eighty numbered lines: more than the viewport shows, so a page press lands
/// in the middle of the document rather than clamping to its end.
fn paged_document() -> String {
    (0..80).map(|index| format!("line{index}")).collect::<Vec<_>>().join("\n")
}

#[test]
fn test_page_down_then_page_up_returns_the_cursor() {
    // The library tests dispatch `Message::PageDown` straight into the editor.
    // What only a UI test covers is the step before that: the canvas reading a
    // real key event, and capturing it rather than letting it through.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&paged_document());

    let _ = ui.tap(Named::PageDown);
    let after_down = ui.cursor();
    assert!(after_down.0 > 0, "page down did not move the cursor");

    let _ = ui.tap(Named::PageUp);

    assert_eq!(ui.cursor(), (0, 0));
}

#[test]
fn test_shift_page_down_selects_from_the_cursor_to_the_new_line() {
    // Whether Shift reached the editor is not visible in the message stream --
    // `carries` compares variants, and `PageDown(true)` and `PageDown(false)`
    // are the same variant. Cutting is what makes the selection observable,
    // the way the Ctrl+X test above does it.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&paged_document());

    let _ = ui.press(Key::Named(Named::PageDown), Modifiers::SHIFT);
    let landed = ui.cursor();
    let _ = ui.press(Key::Character("x".into()), Modifiers::COMMAND);

    // Everything above the line the page landed on is gone; the rest stays.
    let expected = (landed.0..80)
        .map(|index| format!("line{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(ui.content(), expected);
}

#[test]
fn test_page_down_without_shift_selects_nothing() {
    // The other half of the same observation: with no Shift there is no
    // selection, so Ctrl+X has nothing to cut and the buffer is untouched.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&paged_document());

    let _ = ui.tap(Named::PageDown);
    let _ = ui.press(Key::Character("x".into()), Modifiers::COMMAND);

    assert_eq!(ui.content(), paged_document());
}

#[test]
fn test_ctrl_page_down_is_left_to_the_application() {
    // The editor declines Ctrl+Page so a host can bind it -- Ctrl+Page Up/Down
    // is the conventional previous/next-tab combination, and tabs are the
    // application's. Declining means not capturing: the demo's own
    // `event::listen()` only ever receives events no widget took.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&paged_document());

    let _ = ui.press(Key::Named(Named::PageDown), Modifiers::CTRL);

    assert_eq!(ui.cursor(), (0, 0), "the editor acted on a host combination");
}

#[test]
fn test_alt_page_down_is_left_to_the_application() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&paged_document());

    let _ = ui.press(Key::Named(Named::PageDown), Modifiers::ALT);

    assert_eq!(ui.cursor(), (0, 0));
}

#[test]
fn test_ctrl_page_up_is_left_to_the_application() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with(&paged_document());
    // Start away from the top, so a Page Up that fired would be visible.
    let _ = ui.tap(Named::PageDown);
    let landed = ui.cursor();

    let _ = ui.press(Key::Named(Named::PageUp), Modifiers::CTRL);

    assert_eq!(ui.cursor(), landed);
}

// ---- Cut / copy / paste ----

#[test]
fn test_ctrl_x_cuts_the_selection_out_of_the_buffer() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");
    let _ = ui.press(Key::Character("a".into()), Modifiers::COMMAND);

    let messages = ui.press(Key::Character("x".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::Cut));
    assert_eq!(ui.content(), "");
}

#[test]
fn test_ctrl_c_copies_without_touching_the_buffer() {
    // Where the text lands is the system clipboard, which the headless
    // simulator has no access to. What a UI test can pin down is that the
    // canvas turns the combo into a copy and leaves the document alone.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");
    let _ = ui.press(Key::Character("a".into()), Modifiers::COMMAND);

    let messages = ui.press(Key::Character("c".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::Copy));
    assert_eq!(ui.content(), "alpha\nbeta");
}

#[test]
fn test_cut_then_paste_round_trips_the_text() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");
    let _ = ui.press(Key::Character("a".into()), Modifiers::COMMAND);
    let _ = ui.press(Key::Character("x".into()), Modifiers::COMMAND);
    assert_eq!(ui.content(), "");

    let messages = ui.press(Key::Character("v".into()), Modifiers::COMMAND);

    // An empty payload is the editor asking for a clipboard read. The
    // runtime answers with a second `Paste` once the read resolves, and
    // that round trip is the one part the simulator cannot run itself, so
    // the test plays the runtime's role and hands back what Cut put there.
    assert!(messages.iter().any(|message| matches!(
        message,
        Message::EditorEvent(_, EditorMessage::Paste(text)) if text.is_empty()
    )));
    assert_eq!(ui.content(), "");

    let editor_id = ui.app.active_tab_id;
    ui.deliver(Message::EditorEvent(
        editor_id,
        EditorMessage::Paste("alpha\nbeta".to_string()),
    ));

    assert_eq!(ui.content(), "alpha\nbeta");
}

#[test]
fn test_paste_inserts_at_the_cursor() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha");
    let _ = ui.tap(Named::End);

    let editor_id = ui.app.active_tab_id;
    ui.deliver(Message::EditorEvent(
        editor_id,
        EditorMessage::Paste("-beta".to_string()),
    ));

    assert_eq!(ui.content(), "alpha-beta");
}

// ---- The focus gate ----

#[test]
fn test_shortcuts_are_ignored_while_the_canvas_is_unfocused() {
    // The focus gate is what keeps the editor from swallowing keystrokes
    // meant for the toolbar's text input, so it deserves its own test.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha\nbeta");
    ui.deliver(Message::TextInputClicked);

    let messages = ui
        .press(Key::Named(Named::ArrowDown), Modifiers::ALT | Modifiers::SHIFT);

    assert!(!carries(&messages, &EditorMessage::DuplicateLineDown));
    assert_eq!(ui.content(), "alpha\nbeta");
}

// ---- Typing ----

#[test]
fn test_typing_inserts_text_at_the_cursor() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("");

    let messages = ui.typewrite("hello");

    let typed: String = messages
        .iter()
        .filter_map(|message| match message {
            Message::EditorEvent(_, EditorMessage::CharacterInput(ch)) => {
                Some(*ch)
            }
            _ => None,
        })
        .collect();
    assert_eq!(typed, "hello", "every keystroke must reach the editor");
    assert_eq!(ui.content(), "hello");
    assert_eq!(ui.cursor(), (0, 5));
}

#[test]
fn test_enter_starts_a_new_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("");

    let _ = ui.typewrite("one");
    let messages = ui.tap(Named::Enter);
    let _ = ui.typewrite("two");

    assert!(carries(&messages, &EditorMessage::Enter));
    assert_eq!(ui.content(), "one\ntwo");
    assert_eq!(ui.cursor(), (1, 3));
}

#[test]
fn test_typing_a_whole_paragraph_line_by_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("");

    for (index, line) in ["one", "two", "three"].into_iter().enumerate() {
        if index > 0 {
            let _ = ui.tap(Named::Enter);
        }
        let _ = ui.typewrite(line);
    }

    assert_eq!(ui.content(), "one\ntwo\nthree");
    assert_eq!(ui.cursor(), (2, 5));
}

#[test]
fn test_typing_in_the_middle_of_a_line_splices_the_text() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("ac");
    let _ = ui.tap(Named::ArrowRight);

    let _ = ui.typewrite("b");

    assert_eq!(ui.content(), "abc");
}

// ---- Auto-indentation ----

#[test]
fn test_enter_carries_the_indentation_over_to_the_new_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("    indented");
    let _ = ui.tap(Named::End);

    let _ = ui.tap(Named::Enter);
    let _ = ui.typewrite("next");

    assert_eq!(ui.content(), "    indented\n    next");
    assert_eq!(ui.cursor(), (1, 8));
}

#[test]
fn test_enter_adds_no_indentation_after_a_flush_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("flush");
    let _ = ui.tap(Named::End);

    let _ = ui.tap(Named::Enter);
    let _ = ui.typewrite("next");

    assert_eq!(ui.content(), "flush\nnext");
}

#[test]
fn test_turning_auto_indentation_off_from_the_options_panel() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("    indented");

    ui.toggle_option("Auto-indentation");
    assert!(
        ui.app.tabs.iter().all(|tab| !tab.editor.auto_indent_enabled()),
        "the checkbox must have turned the setting off"
    );

    let _ = ui.tap(Named::End);
    let _ = ui.tap(Named::Enter);
    let _ = ui.typewrite("next");

    assert_eq!(ui.content(), "    indented\nnext");
}

// ---- Toggle comment ----

#[test]
fn test_ctrl_slash_comments_the_current_line() {
    // The demo's editors are Lua, so the line comment token is `--`.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("print(1)");

    let messages = ui.press(Key::Character("/".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::ToggleComment));
    assert_eq!(ui.content(), "-- print(1)");
}

#[test]
fn test_ctrl_slash_twice_restores_the_line() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("print(1)");

    let _ = ui.press(Key::Character("/".into()), Modifiers::COMMAND);
    let _ = ui.press(Key::Character("/".into()), Modifiers::COMMAND);

    assert_eq!(ui.content(), "print(1)");
}

#[test]
fn test_commenting_keeps_the_indentation_ahead_of_the_token() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("    x = 1");

    let _ = ui.press(Key::Character("/".into()), Modifiers::COMMAND);

    assert_eq!(ui.content(), "    -- x = 1");
}

#[test]
fn test_ctrl_slash_comments_every_line_of_the_selection() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("one\ntwo");
    let _ = ui.press(Key::Character("a".into()), Modifiers::COMMAND);

    let _ = ui.press(Key::Character("/".into()), Modifiers::COMMAND);

    assert_eq!(ui.content(), "-- one\n-- two");
}

#[test]
fn test_ctrl_slash_works_on_an_azerty_layout() {
    // `/` is Shift+`:` there, so the character only reaches the app through
    // `modified_key`. Matching on `key` alone would silently drop the
    // shortcut for every AZERTY user.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("print(1)");

    let messages = ui.press_as(
        Key::Character(":".into()),
        Key::Character("/".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    assert!(carries(&messages, &EditorMessage::ToggleComment));
    assert_eq!(ui.content(), "-- print(1)");
}

// ---- Auto-closing brackets ----

#[test]
fn test_typing_an_opening_bracket_inserts_its_pair() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("");

    let _ = ui.typewrite("(");

    assert_eq!(ui.content(), "()");
    assert_eq!(ui.cursor(), (0, 1), "the cursor must land between the pair");
}

#[test]
fn test_every_managed_pair_closes_itself() {
    for (opening, expected) in
        [("(", "()"), ("[", "[]"), ("{", "{}"), ("\"", "\"\""), ("'", "''")]
    {
        let (mut app, _) = DemoApp::new();
        let mut ui = Ui::new(&mut app);
        ui.open_editor_with("");

        let _ = ui.typewrite(opening);

        assert_eq!(ui.content(), expected, "typing {opening:?}");
    }
}

#[test]
fn test_typing_the_closing_bracket_steps_over_the_inserted_one() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("");

    let _ = ui.typewrite("(");
    let _ = ui.typewrite(")");

    assert_eq!(ui.content(), "()", "the closer must not be duplicated");
    assert_eq!(ui.cursor(), (0, 2));
}

#[test]
fn test_typing_a_bracket_over_a_selection_wraps_it() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("word");
    let _ = ui.press(Key::Character("a".into()), Modifiers::COMMAND);

    let _ = ui.typewrite("(");

    assert_eq!(ui.content(), "(word)");
}

#[test]
fn test_no_pair_is_inserted_before_a_word() {
    // Auto-close only fires when the cursor is at the end of the line or in
    // front of whitespace or another closer, so that typing a bracket ahead
    // of existing text does not litter the line with stray closers.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("word");

    let _ = ui.typewrite("(");

    assert_eq!(ui.content(), "(word");
}

#[test]
fn test_turning_auto_close_off_from_the_options_panel() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("");

    ui.toggle_option("Auto-close brackets");
    assert!(
        ui.app.tabs.iter().all(|tab| !tab.editor.auto_close_brackets()),
        "the checkbox must have turned the setting off"
    );

    let _ = ui.typewrite("(");

    assert_eq!(ui.content(), "(");
}
