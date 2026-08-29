//! Interface tests for the overlay dialogs: opening and closing search,
//! searching, replacing, and the command palette.

use super::*;

// ---- Opening and closing the search dialog ----

#[test]
fn test_ctrl_f_opens_the_search_dialog() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    assert!(!ui.shows(SEARCH_FIELD));

    let messages = ui.press(Key::Character("f".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::OpenSearch));
    assert!(ui.shows(SEARCH_FIELD));
    assert!(!ui.shows(REPLACE_FIELD), "plain search has no replace field");
}

#[test]
fn test_ctrl_h_opens_the_dialog_in_replace_mode() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");

    let messages = ui.press(Key::Character("h".into()), Modifiers::COMMAND);

    assert!(carries(&messages, &EditorMessage::OpenSearchReplace));
    assert!(ui.shows(SEARCH_FIELD));
    assert!(ui.shows(REPLACE_FIELD));
}

#[test]
fn test_escape_closes_the_search_dialog() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    assert!(ui.shows(SEARCH_FIELD));

    let messages = ui.tap(Named::Escape);

    assert!(carries(&messages, &EditorMessage::CloseSearch));
    assert!(!ui.shows(SEARCH_FIELD));
}

#[test]
fn test_search_stays_shut_while_the_option_is_off() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");

    ui.toggle_option("Allow search/replace");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);

    assert!(!ui.shows(SEARCH_FIELD));
}

#[test]
fn test_reopening_the_dialog_keeps_the_previous_query() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "beta");
    let _ = ui.tap(Named::Escape);

    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);

    assert!(ui.shows("beta"), "the field must come back filled in");
}

// ---- Searching ----

#[test]
fn test_typing_a_query_reports_the_match_count() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);

    ui.type_into(SEARCH_FIELD, "alpha");

    assert!(ui.shows("alpha"), "the field must hold the whole query");
    assert!(ui.shows("1/2"), "the counter must show match 1 of 2");
}

#[test]
fn test_a_query_that_matches_nothing_reports_zero() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    // An empty query renders no counter at all, so "0" appearing has to be
    // the dialog reporting the miss.
    assert!(!ui.shows("0"));

    ui.type_into(SEARCH_FIELD, "zzz");

    assert!(ui.shows("zzz"), "the query must reach the field");
    assert!(ui.shows("0"));
}

#[test]
fn test_f3_steps_to_the_next_match() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    assert!(ui.shows("1/2"));

    let messages = ui.tap(Named::F3);

    assert!(carries(&messages, &EditorMessage::FindNext));
    assert!(ui.shows("2/2"));
}

#[test]
fn test_shift_f3_steps_back_to_the_previous_match() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    let _ = ui.tap(Named::F3);
    assert!(ui.shows("2/2"));

    let messages = ui.press(Key::Named(Named::F3), Modifiers::SHIFT);

    assert!(carries(&messages, &EditorMessage::FindPrevious));
    assert!(ui.shows("1/2"));
}

#[test]
fn test_the_next_match_button_advances_the_counter() {
    // The first icon button under the query field is "previous match", the
    // second is "next match".
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    assert!(ui.shows("1/2"));

    let messages = ui.click_icon_under("alpha", 1);

    assert!(carries(&messages, &EditorMessage::FindNext));
    assert!(ui.shows("2/2"));
}

#[test]
fn test_the_previous_match_button_steps_back() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    let _ = ui.tap(Named::F3);
    assert!(ui.shows("2/2"));

    let messages = ui.click_icon_under("alpha", 0);

    assert!(carries(&messages, &EditorMessage::FindPrevious));
    assert!(ui.shows("1/2"));
}

// ---- Replacing ----

#[test]
fn test_the_replace_button_replaces_only_the_current_match() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    ui.type_into(REPLACE_FIELD, "gamma");

    let messages = ui.click_icon_under("gamma", 0);

    assert!(carries(&messages, &EditorMessage::ReplaceNext));
    assert_eq!(ui.content(), "gamma beta alpha");
}

#[test]
fn test_the_replace_all_button_replaces_every_match() {
    // The second icon button under the replacement field.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    ui.type_into(REPLACE_FIELD, "gamma");

    let messages = ui.click_icon_under("gamma", 1);

    assert!(carries(&messages, &EditorMessage::ReplaceAll));
    assert_eq!(ui.content(), "gamma beta gamma");
}

#[test]
fn test_replacing_can_be_undone_in_one_step() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    ui.type_into(REPLACE_FIELD, "gamma");
    let _ = ui.click_icon_under("gamma", 1);
    assert_eq!(ui.content(), "gamma beta gamma");

    let _ = ui.press(Key::Character("z".into()), Modifiers::COMMAND);

    assert_eq!(ui.content(), "alpha beta alpha");
}

// ---- Command palette ----

#[test]
fn test_ctrl_shift_p_opens_the_command_palette() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    assert!(!ui.shows(PALETTE_FIELD));

    let messages = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    assert!(carries(&messages, &EditorMessage::OpenCommandPalette));
    assert!(ui.shows(PALETTE_FIELD));
    assert!(ui.shows("Fold All"), "the built-in commands must be listed");
    assert!(ui.shows("Open File"), "the app's own commands must be listed");
}

#[test]
fn test_escape_closes_the_command_palette() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );
    assert!(ui.shows(PALETTE_FIELD));

    let messages = ui.tap(Named::Escape);

    assert!(carries(&messages, &EditorMessage::CloseCommandPalette));
    assert!(!ui.shows(PALETTE_FIELD));
}

#[test]
fn test_typing_narrows_the_command_list() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );
    assert!(ui.shows("Move Line Up"));

    ui.type_into(PALETTE_FIELD, "fold all");

    assert!(ui.shows("Fold All"));
    assert!(!ui.shows("Move Line Up"), "non-matching rows must be dropped");
}

#[test]
fn test_a_query_matching_nothing_says_so() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    ui.type_into(PALETTE_FIELD, "zzz");

    assert!(ui.shows("No matching command"));
    assert!(!ui.shows("Fold All"));
}

#[test]
fn test_clicking_a_command_runs_it_and_closes_the_palette() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );
    ui.type_into(PALETTE_FIELD, "fold all");

    let messages = ui.click("Fold All");

    assert!(carries(&messages, &EditorMessage::CommandPaletteSelected(0)));
    assert!(!ui.shows(PALETTE_FIELD), "running a command closes the palette");
}

#[test]
fn test_the_palette_forwards_the_apps_own_commands() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );
    ui.type_into(PALETTE_FIELD, "new tab");
    assert!(ui.shows("New Tab"));

    let messages = ui.click("New Tab");

    assert!(carries(&messages, &EditorMessage::CommandPaletteSelected(0)));
    assert!(!ui.shows(PALETTE_FIELD));
}

#[test]
fn test_the_palette_stays_shut_while_the_option_is_off() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    if let Some(tab) = ui.app.get_active_tab() {
        tab.editor.set_command_palette_enabled(false);
    }

    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    assert!(!ui.shows(PALETTE_FIELD));
}

#[test]
fn test_the_arrow_keys_move_through_the_palette_instead_of_the_buffer() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1\nlocal y = 2");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    let messages = ui.tap(Named::ArrowDown);

    assert!(carries(&messages, &EditorMessage::CommandPaletteNavigate(true)));
    assert_eq!(ui.cursor(), (0, 0), "the caret must not move in the buffer");
}

#[test]
fn test_typing_a_query_goes_to_the_palette_instead_of_the_buffer() {
    // The character-input counterpart of the arrow-key test above, and the
    // easier half of the pair: the arrow keys need the palette's own canvas
    // listener to be taken from the focused query field, while plain
    // characters are captured by that field before the editor canvas is
    // reached. So this asserts an integration rather than a branch of ours —
    // no mutation of this crate makes it fail — but it is the property a user
    // would notice breaking, and it pins the layering that provides it.
    // `type_into` drives the real widget tree and applies every message it
    // produces, so a leak would land in the buffer here as it would for a user.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("local x = 1");
    let _ = ui.press(
        Key::Character("p".into()),
        Modifiers::COMMAND | Modifiers::SHIFT,
    );

    ui.type_into(PALETTE_FIELD, "fold all");

    // Both halves of the claim: the keystrokes reached the palette, and they
    // reached nothing else. The space matters as much as the letters — it is
    // the one character that would be hardest to spot in a narrowed list.
    assert!(ui.shows("Fold All"), "the query must reach the palette");
    assert_eq!(
        ui.content(),
        "local x = 1",
        "the query must not reach the buffer"
    );
    assert_eq!(ui.cursor(), (0, 0), "the caret must not move in the buffer");
}
