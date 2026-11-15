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
    assert!(ui.shows("0/0"));
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
    // The first navigation button to the right of the query field is "previous match", the
    // second is "next match".
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    assert!(ui.shows("1/2"));

    let messages = ui.click_control("search-next");

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

    let messages = ui.click_control("search-previous");

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

    let messages = ui.click_control("search-replace-next");

    assert!(carries(&messages, &EditorMessage::ReplaceNext));
    assert_eq!(ui.content(), "gamma beta alpha");
}

#[test]
fn test_the_replace_all_button_replaces_every_match() {
    // The second action button to the right of the replacement field.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "alpha");
    ui.type_into(REPLACE_FIELD, "gamma");

    let messages = ui.click_control("search-replace-all");

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
    let _ = ui.click_control("search-replace-all");
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

#[test]
fn toolbar_fields_and_buttons_are_aligned_in_both_input_modes()
-> Result<(), Box<dyn std::error::Error>> {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("alpha beta alpha");
    let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
    for multiline in [false, true] {
        if multiline {
            let _ = ui.click_control("search-multiline");
        }
        let mut sim = simulator(view(ui.app));
        let mut bounds = |id| {
            sim.find(iced_test::selector::id(id))
                .map(|target| target.bounds())
                .unwrap_or_default()
        };
        let query = bounds("search-query-field");
        let replacement = bounds("search-replacement-field");
        assert!((query.x - replacement.x).abs() < 0.1);
        assert!((query.width - replacement.width).abs() < 0.1);
        assert!((query.height - replacement.height).abs() < 0.1);
        let toggle = bounds("search-replace-toggle");
        assert!(toggle.x > query.x + query.width);
        for id in [
            "search-case",
            "search-word",
            "search-regex",
            "search-multiline",
            "search-select-all",
            "search-previous",
            "search-next",
            "search-close",
        ] {
            let button = bounds(id);
            assert!(
                (button.center_y() - toggle.center_y()).abs() < 0.1,
                "{id} is misaligned"
            );
        }
        let replace_next = bounds("search-replace-next");
        let replace_all = bounds("search-replace-all");
        assert!((replace_next.x - toggle.x).abs() < 0.1);
        assert!((replace_all.x - bounds("search-multiline").x).abs() < 0.1);
        assert!((replace_next.center_y() - replacement.y - 14.0).abs() < 0.1);
    }
    let messages = ui.click_control("search-replace-toggle");
    assert!(carries(&messages, &EditorMessage::ToggleSearchReplace));
    // The replacement row is removed without changing the search controls.
    let mut sim = simulator(view(ui.app));
    let replacement =
        sim.find(iced_test::selector::id("search-replacement-field"));
    assert!(replacement.err().is_some());
    sim.find(iced_test::selector::id("search-replace-toggle"))?;
    Ok(())
}

#[test]
fn toolbar_match_options_remain_clickable_with_vector_icons() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("cat CAT scatter");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "cat");
    assert!(ui.shows("1/3"));
    assert!(carries(
        &ui.click_control("search-case"),
        &EditorMessage::ToggleCaseSensitive
    ));
    assert!(ui.shows("1/2"));
    assert!(carries(
        &ui.click_control("search-word"),
        &EditorMessage::ToggleWholeWord
    ));
    assert!(ui.shows("1/1"));
    assert!(carries(
        &ui.click_control("search-regex"),
        &EditorMessage::ToggleSearchRegex
    ));
}

#[test]
fn expanded_search_accepts_newlines_without_editing_the_document() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("a\nb a\nb");
    let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
    let _ = ui.click_control("search-multiline");
    let messages = {
        let mut sim = simulator(view(ui.app));
        let bounds = sim
            .find(iced_test::selector::id("search-query-field"))
            .map(|target| target.bounds())
            .unwrap_or_default();
        sim.point_at(Point::new(bounds.x + 12.0, bounds.y + 12.0));
        let _ = sim.simulate(iced_test::simulator::click());
        let _ = sim.simulate(iced_test::simulator::typewrite("a"));
        let _ = sim
            .simulate(key_combo(Key::Named(Named::Enter), Modifiers::empty()));
        let _ = sim.simulate(iced_test::simulator::typewrite("b"));
        sim.into_messages().collect::<Vec<_>>()
    };
    assert!(carries(
        &messages,
        &EditorMessage::SearchInputAction(
            iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Enter
            )
        )
    ));
    for message in messages {
        let _ = ui.app.update(message);
    }
    assert!(ui.shows("1/2"));
    assert_eq!(ui.content(), "a\nb a\nb");
}

#[test]
fn select_all_search_matches_button_returns_typing_to_all_matches() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("foo bar foo\nfoo");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "foo");
    let messages = ui.click_control("search-select-all");
    assert!(carries(&messages, &EditorMessage::SelectAllSearchMatches));
    assert!(ui.shows("foo"), "the search input must remain visible");
    let _ = ui.typewrite("x");
    assert!(ui.shows("foo"), "typing must preserve the search query");
    assert!(ui.shows("0/0"));
    assert_eq!(ui.content(), "x bar x\nx");
    let _ = ui.press(Key::Character("z".into()), Modifiers::COMMAND);
    assert_eq!(ui.content(), "foo bar foo\nfoo");
}

#[test]
fn alt_enter_selects_matches_from_focused_single_and_multiline_inputs() {
    for multiline in [false, true] {
        let (mut app, _) = DemoApp::new();
        let mut ui = Ui::new(&mut app);
        ui.open_editor_with("foo bar foo");
        let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
        ui.type_into(SEARCH_FIELD, "foo");
        if multiline {
            let _ = ui.click_control("search-multiline");
        }
        let messages = {
            let mut sim = simulator(view(ui.app));
            let field = sim
                .find(iced_test::selector::id("search-query-field"))
                .map(|target| target.bounds())
                .unwrap_or_default();
            sim.point_at(Point::new(field.x + 12.0, field.y + 12.0));
            let _ = sim.simulate(iced_test::simulator::click());
            let _ = sim
                .simulate(key_combo(Key::Named(Named::Enter), Modifiers::ALT));
            sim.into_messages().collect::<Vec<_>>()
        };
        assert!(carries(&messages, &EditorMessage::SelectAllSearchMatches));
        assert!(!carries(&messages, &EditorMessage::FindNext));
        for message in messages {
            let _ = ui.app.update(message);
        }
        {
            let mut sim = simulator(view(ui.app));
            let field = sim.find(iced_test::selector::id("search-query-field"));
            assert!(field.is_ok(), "selection must keep search visible");
        }
        let _ = ui.typewrite("x");
        assert!(ui.shows("0/0"));
        assert_eq!(ui.content(), "x bar x");
    }
}

fn press_in_search_field(
    ui: &mut Ui<'_>,
    field: &'static str,
    key: Key,
    modifiers: Modifiers,
) -> Vec<Message> {
    let messages = {
        let mut sim = simulator(view(ui.app));
        let bounds = sim
            .find(iced_test::selector::id(field))
            .map(|target| target.bounds())
            .unwrap_or_default();
        sim.point_at(Point::new(bounds.x + 12.0, bounds.y + 12.0));
        let _ = sim.simulate(iced_test::simulator::click());
        let _ = sim.simulate(key_combo(key, modifiers));
        sim.into_messages().collect::<Vec<_>>()
    };
    for message in &messages {
        let _ = ui.app.update(message.clone());
    }
    messages
}

#[test]
fn toolbar_option_shortcuts_work_inside_both_input_widgets() {
    let options = Modifiers::ALT
        | if cfg!(target_os = "macos") {
            Modifiers::COMMAND
        } else {
            Modifiers::empty()
        };
    for multiline in [false, true] {
        let (mut app, _) = DemoApp::new();
        let mut ui = Ui::new(&mut app);
        ui.open_editor_with("foo FOO foo");
        let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
        ui.type_into(SEARCH_FIELD, "foo");
        if multiline {
            let _ = ui.click_control("search-multiline");
        }
        for (key, expected) in [
            ("c", EditorMessage::ToggleCaseSensitive),
            ("w", EditorMessage::ToggleWholeWord),
            ("r", EditorMessage::ToggleSearchRegex),
            ("m", EditorMessage::ToggleSearchMultiline),
        ] {
            let messages = press_in_search_field(
                &mut ui,
                "search-query-field",
                Key::Character(key.into()),
                options,
            );
            assert!(
                carries(&messages, &expected),
                "{key} in multiline={multiline}"
            );
            assert!(!carries(&messages, &EditorMessage::Copy));
        }
        let toggle = Modifiers::COMMAND
            | if cfg!(target_os = "macos") {
                Modifiers::SHIFT
            } else {
                Modifiers::empty()
            };
        for _ in 0..2 {
            let messages = press_in_search_field(
                &mut ui,
                "search-query-field",
                Key::Character("h".into()),
                toggle,
            );
            assert!(carries(&messages, &EditorMessage::ToggleSearchReplace));
        }
        assert_eq!(ui.content(), "foo FOO foo");
    }
}

#[test]
fn replace_shortcuts_work_in_single_and_multiline_replacement_fields() {
    for multiline in [false, true] {
        let (mut app, _) = DemoApp::new();
        let mut ui = Ui::new(&mut app);
        ui.open_editor_with("foo bar foo");
        let _ = ui.press(Key::Character("h".into()), Modifiers::COMMAND);
        ui.type_into(SEARCH_FIELD, "foo");
        ui.type_into(REPLACE_FIELD, "gamma");
        if multiline {
            let _ = ui.click_control("search-multiline");
        }
        let modifiers = if multiline {
            Modifiers::COMMAND | Modifiers::SHIFT
        } else {
            Modifiers::empty()
        };
        let messages = press_in_search_field(
            &mut ui,
            "search-replacement-field",
            Key::Named(Named::Enter),
            modifiers,
        );
        assert!(carries(&messages, &EditorMessage::ReplaceNext));
        assert_eq!(ui.content(), "gamma bar foo");
        let messages = press_in_search_field(
            &mut ui,
            "search-replacement-field",
            Key::Named(Named::Enter),
            Modifiers::COMMAND,
        );
        assert!(carries(&messages, &EditorMessage::ReplaceAll));
        assert!(!carries(&messages, &EditorMessage::ReplaceNext));
        assert_eq!(ui.content(), "gamma bar gamma");
    }
}

#[test]
fn match_navigation_shortcuts_override_goto_line_only_during_search() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.open_editor_with("foo bar foo");
    let _ = ui.press(Key::Character("f".into()), Modifiers::COMMAND);
    ui.type_into(SEARCH_FIELD, "foo");
    for multiline in [false, true] {
        if multiline {
            let _ = ui.click_control("search-multiline");
        }
        let key = if cfg!(target_os = "macos") {
            Key::Character("g".into())
        } else {
            Key::Named(Named::F3)
        };
        let mods = if cfg!(target_os = "macos") {
            Modifiers::COMMAND
        } else {
            Modifiers::empty()
        };
        let messages = press_in_search_field(
            &mut ui,
            "search-query-field",
            key.clone(),
            mods,
        );
        assert!(carries(&messages, &EditorMessage::FindNext));
        assert!(!carries(&messages, &EditorMessage::OpenGotoLine));
        assert!(ui.shows("2/2"));
        let messages = press_in_search_field(
            &mut ui,
            "search-query-field",
            key,
            mods | Modifiers::SHIFT,
        );
        assert!(carries(&messages, &EditorMessage::FindPrevious));
        assert!(ui.shows("1/2"));
    }
}
