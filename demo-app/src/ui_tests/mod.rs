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

mod chrome;
mod dialogs;
mod editing;
mod sticky_scroll;

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
