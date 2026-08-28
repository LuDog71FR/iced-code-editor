//! End-to-end interface tests driven through the real [`crate::ui::view`]
//! widget tree.
//!
//! The rest of the demo app's tests call `DemoApp::update` (or a handler)
//! with a hand-built [`Message`], which proves the handler is correct but
//! says nothing about whether any widget actually emits that message. These
//! tests close that gap: they render `view` in Iced's headless
//! [`Simulator`](iced_test::Simulator), click and type on the real widgets,
//! feed the resulting messages back into `update`, and assert on the state
//! and on what the next render shows.
//!
//! Scope: the simulator only sees the widget tree, never
//! [`DemoApp::subscription`](crate::app::DemoApp::subscription). Shortcuts
//! routed through the global event stream — the Escape handling in
//! `app.rs`, for instance — are covered by `update`-level tests in
//! `app/app_lsp.rs` instead.

use crate::app::{DemoApp, Message};
use crate::ui::view;
use iced::keyboard::key::Named;
use iced::keyboard::{self, Key, Modifiers};
use iced::{Event, Point, Rectangle, mouse};
use iced_code_editor::Message as EditorMessage;
use iced_test::selector::Candidate;
use iced_test::simulator;
use std::sync::{Mutex, MutexGuard};

/// Serialises the interface tests against each other.
///
/// `CodeEditor` tracks which editor holds the keyboard in a process-wide
/// global, so two tests clicking their own canvas concurrently would steal
/// focus from one another — and a test that lost the race would see its
/// keystrokes silently dropped by the focus gate rather than fail loudly.
/// The lock is therefore held for a whole test rather than per render, and
/// [`Ui`] is the only way to take it — see [`Ui::new`].
///
/// It also keeps headless renderers from being built concurrently, which
/// segfaults on the wgpu backend. `.cargo/config.toml` points the tests at
/// the software rasteriser instead, so that hazard is normally out of
/// reach, but the lock covers anyone who overrides the backend.
static UI: Mutex<()> = Mutex::new(());

/// Exclusive, stateful access to the app's interface for one test.
///
/// Every method renders [`view`] afresh, replays some input against it, and
/// feeds the messages that came back into `DemoApp::update` — one turn of
/// the Iced loop, the same sequence the runtime performs.
struct Ui<'a> {
    /// The application under test.
    app: &'a mut DemoApp,
    /// Held for the lifetime of the test. See [`UI`].
    _guard: MutexGuard<'static, ()>,
}

impl<'a> Ui<'a> {
    /// Takes exclusive access to the interface for the rest of the test.
    ///
    /// Recovers from a poisoned lock: a panicking test leaves no shared
    /// state behind, and failing every later test on top of the first one
    /// would only bury the real failure.
    fn new(app: &'a mut DemoApp) -> Self {
        Self {
            app,
            _guard: UI.lock().unwrap_or_else(|error| error.into_inner()),
        }
    }

    /// Renders, replays `events`, applies the messages, and returns them.
    fn interact(&mut self, events: Vec<Event>) -> Vec<Message> {
        let messages: Vec<Message> = {
            let mut ui = simulator(view(self.app));
            let _ = ui.simulate(events);
            ui.into_messages().collect()
        };

        for message in &messages {
            let _ = self.app.update(message.clone());
        }
        messages
    }

    /// Clicks the widget labelled `label`, returning the messages produced.
    ///
    /// Fails the test when no visible widget carries that exact label,
    /// which is the point: a renamed or removed button must break the test
    /// that covers it rather than silently produce an empty message list.
    fn click(&mut self, label: &str) -> Vec<Message> {
        let messages: Vec<Message> = {
            let mut ui = simulator(view(self.app));
            assert!(
                ui.click(label).is_ok(),
                "no clickable widget labelled {label:?}"
            );
            ui.into_messages().collect()
        };

        for message in &messages {
            let _ = self.app.update(message.clone());
        }
        messages
    }

    /// Whether a widget labelled `label` is in the current render.
    fn shows(&mut self, label: &str) -> bool {
        let mut ui = simulator(view(self.app));
        ui.find(label).is_ok()
    }

    /// Presses `key` with `modifiers` held, returning the messages produced.
    fn press(&mut self, key: Key, modifiers: Modifiers) -> Vec<Message> {
        self.interact(key_combo(key, modifiers))
    }

    /// Presses a named key with no modifier held.
    fn tap(&mut self, key: Named) -> Vec<Message> {
        self.press(Key::Named(key), Modifiers::NONE)
    }

    /// Presses a key whose character differs from the key's own label.
    ///
    /// A French AZERTY keyboard reaches `/` through Shift+`:`, and only
    /// `modified_key` carries the resulting character — the layout trap the
    /// editor's `is_key_char` exists to avoid.
    fn press_as(
        &mut self,
        key: Key,
        modified_key: Key,
        modifiers: Modifiers,
    ) -> Vec<Message> {
        self.interact(key_combo_as(key, modified_key, modifiers))
    }

    /// Types `text` on the keyboard, one character at a time.
    fn typewrite(&mut self, text: &str) -> Vec<Message> {
        self.interact(iced_test::simulator::typewrite(text).collect())
    }

    /// The active tab's cursor, as `(line, column)`.
    fn cursor(&self) -> (usize, usize) {
        self.app
            .tabs
            .iter()
            .find(|tab| tab.id == self.app.active_tab_id)
            .map(|tab| tab.editor.cursor_position())
            .unwrap_or_default()
    }

    /// Opens the editor options panel and clicks the toggle named `label`.
    ///
    /// The checkboxes only exist once the panel is expanded, so both clicks
    /// are needed — and both are the ones a user performs.
    fn toggle_option(&mut self, label: &str) {
        let _ = self.click("Options ▼");
        let _ = self.click(label);
    }

    /// Types `text` into the search-dialog field that currently reads
    /// `label` (its placeholder while empty, otherwise its contents).
    ///
    /// One character per render, because an Iced `text_input` derives every
    /// edit from the value it was handed when it was built — a whole string
    /// replayed against a single render would leave only its last character
    /// behind. The field is re-clicked before each character too, since
    /// `Simulator` builds a fresh widget tree every time and nothing keeps
    /// the input focused between them.
    fn type_into(&mut self, label: &str, text: &str) {
        let mut typed = String::new();

        for character in text.chars() {
            let reading = if typed.is_empty() {
                label.to_string()
            } else {
                typed.clone()
            };

            let messages: Vec<Message> = {
                let mut ui = simulator(view(self.app));
                let found = ui.find(input_reading(reading.clone()));
                assert!(found.is_ok(), "no text input reading {reading:?}");
                let bounds = found.unwrap_or_default();
                // Near the right edge, so the caret lands past whatever is
                // already there and each character appends.
                ui.point_at(Point::new(
                    bounds.x + bounds.width - 6.0,
                    bounds.y + bounds.height / 2.0,
                ));
                let _ = ui.simulate(iced_test::simulator::click());
                let _ = ui.simulate(iced_test::simulator::typewrite(
                    character.encode_utf8(&mut [0u8; 4]),
                ));
                ui.into_messages().collect()
            };

            for message in &messages {
                let _ = self.app.update(message.clone());
            }
            typed.push(character);
        }
    }

    /// Clicks the icon button at `index` in the row directly under the
    /// dialog field reading `label`.
    ///
    /// The dialog's actions are icon-only Font Awesome buttons: they carry
    /// no text node, so the selector the other tests use cannot see them.
    /// They do sit in a left-aligned row immediately below their own input,
    /// which makes that input a findable anchor to measure from.
    fn click_icon_under(&mut self, label: &str, index: usize) -> Vec<Message> {
        let messages: Vec<Message> = {
            let mut ui = simulator(view(self.app));
            let found = ui.find(input_reading(label.to_string()));
            assert!(found.is_ok(), "no text input reading {label:?}");
            let input = found.unwrap_or_default();
            ui.point_at(Point::new(
                input.x
                    + ICON_BUTTON_SIZE / 2.0
                    + index as f32 * ICON_BUTTON_STEP,
                input.y + input.height + ICON_ROW_GAP + ICON_BUTTON_SIZE / 2.0,
            ));
            let _ = ui.simulate(iced_test::simulator::click());
            ui.into_messages().collect()
        };

        assert!(
            !messages.is_empty(),
            "icon button {index} under {label:?} was not hit — has the \
             dialog's layout moved?"
        );
        for message in &messages {
            let _ = self.app.update(message.clone());
        }
        messages
    }

    /// Applies `message` directly, standing in for the Iced runtime.
    ///
    /// Only for the steps a headless test cannot perform itself, such as
    /// delivering the clipboard contents the editor asked for.
    fn deliver(&mut self, message: Message) {
        let _ = self.app.update(message);
    }

    /// The active tab's text.
    fn content(&self) -> String {
        self.app
            .tabs
            .iter()
            .find(|tab| tab.id == self.app.active_tab_id)
            .map(|tab| tab.editor.content())
            .unwrap_or_default()
    }

    /// Loads `content` into the active tab, then clicks the canvas to give
    /// it the keyboard and puts the cursor at the start of the document.
    ///
    /// `CodeEditor` drops every keyboard event unless its canvas holds
    /// focus, and only a real mouse click turns that on, so the click is
    /// not skippable. `Ctrl/Cmd+Home` afterwards pins the cursor down,
    /// because where a click lands depends on the font the headless
    /// renderer happened to pick.
    fn open_editor_with(&mut self, content: &str) {
        if let Some(tab) = self.app.get_active_tab() {
            let _ = tab.editor.reset(content);
        }

        let messages: Vec<Message> = {
            let mut ui = simulator(view(self.app));
            let found = ui.find(code_canvas);
            assert!(found.is_ok(), "the code canvas must be rendered");
            // Just inside the code area: the gutter takes the first 45px.
            let bounds = found.unwrap_or_default();
            ui.point_at(Point::new(bounds.x + 80.0, bounds.y + 8.0));
            let _ = ui.simulate(iced_test::simulator::click());
            ui.into_messages().collect()
        };
        for message in &messages {
            let _ = self.app.update(message.clone());
        }
        assert!(
            carries(&messages, &EditorMessage::MouseClick(Point::ORIGIN)),
            "the click must reach the canvas"
        );

        let _ = self.press(Key::Named(Named::Home), Modifiers::COMMAND);
        assert_eq!(self.content(), content);
        assert_eq!(
            self.app.tabs.first().map(|tab| tab.editor.cursor_position()),
            Some((0, 0)),
            "the editor must be focused and its cursor homed"
        );
    }

    /// Scrolls the code canvas down by `lines` mouse-wheel notches.
    ///
    /// The wheel is the only way a headless test can move the viewport:
    /// `scrollable::Viewport` has no public constructor, so the editor's
    /// `Scrolled` message cannot be forged by hand.
    fn scroll_code_canvas(&mut self, lines: f32) {
        let messages: Vec<Message> = {
            let mut ui = simulator(view(self.app));
            let found = ui.find(code_canvas);
            assert!(found.is_ok(), "the code canvas must be rendered");
            let bounds = found.unwrap_or_default();
            ui.point_at(Point::new(
                bounds.x + bounds.width / 2.0,
                bounds.y + bounds.height / 2.0,
            ));
            let _ =
                ui.simulate(vec![Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y: -lines },
                })]);
            ui.into_messages().collect()
        };

        for message in &messages {
            let _ = self.app.update(message.clone());
        }
    }

    /// Clicks the code area's very first row, where a pinned sticky header
    /// sits when there is one.
    fn click_top_of_code_area(&mut self) -> Vec<Message> {
        let messages: Vec<Message> = {
            let mut ui = simulator(view(self.app));
            let found = ui.find(code_canvas);
            assert!(found.is_ok(), "the code canvas must be rendered");
            let bounds = found.unwrap_or_default();
            // Past the 45px gutter, on the topmost row of the code area —
            // the same spot `open_editor_with` proves reaches the canvas.
            ui.point_at(Point::new(bounds.x + 80.0, bounds.y + 8.0));
            let _ = ui.simulate(iced_test::simulator::click());
            ui.into_messages().collect()
        };

        for message in &messages {
            let _ = self.app.update(message.clone());
        }
        messages
    }

    /// The active tab's vertical scroll offset, in pixels.
    fn scroll_offset(&self) -> f32 {
        self.app
            .tabs
            .iter()
            .find(|tab| tab.id == self.app.active_tab_id)
            .map(|tab| tab.editor.viewport_scroll())
            .unwrap_or_default()
    }
}

/// Locates the code canvas's viewport.
///
/// `CodeEditor` gives the scrollable wrapping its canvas an id so it can
/// scroll to the cursor, and it is the only scrollable in the demo that has
/// one — the tab bar and the output log are anonymous. The height check
/// keeps the editor's own horizontal scrollbar, which is also identified,
/// from matching when it is on screen.
///
/// Takes its candidate by value because that is the shape `iced_selector`
/// blanket-implements `Selector` for.
#[allow(clippy::needless_pass_by_value)]
fn code_canvas(candidate: Candidate<'_>) -> Option<Rectangle> {
    match candidate {
        Candidate::Scrollable { id: Some(_), bounds, .. }
            if bounds.height > 100.0 =>
        {
            Some(bounds)
        }
        _ => None,
    }
}

/// Builds the events a key combo produces, modifier changes included.
///
/// `iced_test` only ships `tap_key`, which hardcodes no modifiers, so every
/// shortcut here has to be assembled by hand. `key` and `modified_key`
/// carry the same value, which is what a keyboard reports for the letters
/// and arrows used below.
fn key_combo(key: Key, modifiers: Modifiers) -> Vec<Event> {
    key_combo_as(key.clone(), key, modifiers)
}

/// Builds a key combo whose `modified_key` differs from its `key`, which is
/// what a keyboard reports for a character reached through a modifier.
fn key_combo_as(
    key: Key,
    modified_key: Key,
    modifiers: Modifiers,
) -> Vec<Event> {
    let physical_key = keyboard::key::Physical::Unidentified(
        keyboard::key::NativeCode::Unidentified,
    );

    vec![
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: modified_key.clone(),
            physical_key,
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }),
        Event::Keyboard(keyboard::Event::KeyReleased {
            key,
            modified_key,
            physical_key,
            location: keyboard::Location::Standard,
            modifiers,
        }),
        Event::Keyboard(keyboard::Event::ModifiersChanged(Modifiers::NONE)),
    ]
}

/// Whether `messages` forwards an editor message of the same kind as
/// `wanted`, ignoring any payload it carries.
fn carries(messages: &[Message], wanted: &EditorMessage) -> bool {
    messages.iter().any(|message| {
        matches!(message, Message::EditorEvent(_, event)
            if std::mem::discriminant(event) == std::mem::discriminant(wanted))
    })
}

/// Placeholder of the search dialog's query field, as the user sees it.
///
/// The demo runs in English; `Translations::search_placeholder` is the
/// source of this string. Selecting on it is how a person finds the field,
/// and a reworded placeholder should fail these tests loudly.
const SEARCH_FIELD: &str = "Search...";

/// Placeholder of the search dialog's replacement field.
const REPLACE_FIELD: &str = "Replace...";

/// Placeholder of the command palette's filter field, as the user sees it.
///
/// Sourced from `Translations::command_palette_placeholder`, for the same
/// reason as [`SEARCH_FIELD`].
const PALETTE_FIELD: &str = "Type a command...";

/// Side of the dialog's square icon buttons, in pixels.
const ICON_BUTTON_SIZE: f32 = 15.0;

/// Distance between the left edges of two neighbouring icon buttons.
const ICON_BUTTON_STEP: f32 = 18.0;

/// Vertical gap between a dialog field and the button row beneath it.
const ICON_ROW_GAP: f32 = 5.0;

/// Selects the text input whose visible content is `label`.
///
/// Restricted to inputs on purpose: the plain `&str` selector also matches
/// static text, and the demo renders plenty of that above the dialog. An
/// empty input reports its placeholder here, which is what makes a field
/// findable before anything has been typed into it.
fn input_reading(
    label: String,
) -> impl FnMut(Candidate<'_>) -> Option<Rectangle> {
    move |candidate| match candidate {
        Candidate::TextInput { bounds, state, .. } if state.text() == label => {
            Some(bounds)
        }
        _ => None,
    }
}

// ---- Toolbar ----

#[test]
fn test_open_button_emits_open_file() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);

    let messages = ui.click("Open");

    assert!(matches!(messages.as_slice(), [Message::OpenFile]));
}

#[test]
fn test_save_button_emits_save_file() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);

    let messages = ui.click("Save");

    assert!(matches!(messages.as_slice(), [Message::SaveFile]));
}

#[test]
fn test_save_as_button_emits_save_file_as() {
    // "Save" is a prefix of "Save As", so this also pins down that the
    // selector matches the whole label and not just its beginning.
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);

    let messages = ui.click("Save As");

    assert!(matches!(messages.as_slice(), [Message::SaveFileAs]));
}

#[test]
fn test_run_button_logs_the_simulated_execution() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    let before = ui.app.log_messages.len();

    let messages = ui.click("Run");

    assert!(matches!(messages.as_slice(), [Message::RunCode]));
    assert!(
        ui.app.log_messages.len() > before,
        "running code must append to the output pane"
    );
}

#[test]
fn test_new_tab_button_opens_another_tab() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    assert_eq!(ui.app.tabs.len(), 1);

    let messages = ui.click("+ New Tab");

    assert!(matches!(messages.as_slice(), [Message::NewTab]));
    assert_eq!(ui.app.tabs.len(), 2);
    assert_ne!(
        ui.app.active_tab_id,
        crate::types::EditorId(0),
        "the new tab must become the active one"
    );
}

// ---- Settings modal ----

#[test]
fn test_settings_button_opens_the_modal() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    assert!(!ui.app.show_settings);
    // "Close" only exists inside the modal, so it doubles as the marker for
    // whether the modal is on screen.
    assert!(!ui.shows("Close"));

    let messages = ui.click("Settings");

    assert!(matches!(messages.as_slice(), [Message::ToggleSettings]));
    assert!(ui.app.show_settings);
    assert!(ui.shows("Close"), "the modal must now be rendered");
}

#[test]
fn test_modal_close_button_dismisses_the_settings() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.deliver(Message::ToggleSettings);
    assert!(ui.app.show_settings);

    let messages = ui.click("Close");

    assert!(matches!(messages.as_slice(), [Message::ToggleSettings]));
    assert!(!ui.app.show_settings);
    assert!(!ui.shows("Close"), "the modal must be gone");
}

// ---- Tab bar ----

#[test]
fn test_tab_close_button_closes_that_tab() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.deliver(Message::NewTab);
    assert_eq!(ui.app.tabs.len(), 2);
    // `click` takes the first match in tree order, which is the leftmost
    // tab's close button.
    let first_tab_id = ui.app.tabs.first().map(|tab| tab.id);

    let messages = ui.click("×");

    assert!(matches!(messages.as_slice(), [Message::CloseTab(_)]));
    assert_eq!(ui.app.tabs.len(), 1);
    assert!(
        ui.app.tabs.iter().all(|tab| Some(tab.id) != first_tab_id),
        "the closed tab must be the one whose button was clicked"
    );
}

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
