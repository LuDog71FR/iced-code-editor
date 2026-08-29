//! Interface tests for the application chrome: the toolbar buttons, the
//! settings modal, the tab bar, and the language indicator.

use super::*;

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
fn test_copy_button_asks_for_the_log_to_be_copied() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);

    let messages = ui.click("Copy");

    assert!(matches!(messages.as_slice(), [Message::CopyLog]));
}

#[test]
fn test_clear_button_empties_the_output_pane() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);
    ui.app.log_messages.push("[OUTPUT] noise".to_string());

    let messages = ui.click("Clear");

    assert!(matches!(messages.as_slice(), [Message::ClearLog]));
    assert_eq!(ui.app.log_messages, vec!["[INFO] Log cleared".to_string()]);
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

// ---- Language indicator ----

#[test]
fn test_toolbar_shows_the_active_grammar() {
    let (mut app, _) = DemoApp::new();
    let mut ui = Ui::new(&mut app);

    // An untitled tab holds the demo's Lua templates.
    assert!(ui.shows("Lua"));
    assert!(!ui.shows("Rust"));

    let _ = ui.app.update(Message::FileOpened(Ok((
        std::path::PathBuf::from("/tmp/iced-code-editor/indicator.rs"),
        "fn main() {}".to_string(),
    ))));

    assert!(ui.shows("Rust"), "opening a .rs file must re-label the indicator");
    assert!(!ui.shows("Lua"));
}
