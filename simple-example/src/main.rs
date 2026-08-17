//! Minimal example of embedding [`CodeEditor`] in an Iced application.
//!
//! Shows the three things a host has to wire up: forwarding the editor's
//! messages through its own `Message` type, rendering it with `view()`, and
//! releasing editor focus when another widget takes over — without which the
//! editor would keep swallowing keystrokes meant for the text input above it.

use iced::widget::{column, container, mouse_area, text_input};
use iced::{Element, Task};
use iced_code_editor::{CodeEditor, Message as EditorMessage};

/// Application state: the editor plus an unrelated text input, present to
/// demonstrate focus hand-off between the two.
struct MyApp {
    editor: CodeEditor,
    input_value: String,
}

/// Messages handled by the application.
#[derive(Debug, Clone)]
enum Message {
    /// A message emitted by the embedded editor.
    EditorEvent(EditorMessage),
    /// The text input's contents changed.
    InputChanged(String),
    /// The text input was clicked, which must take focus from the editor.
    TextInputClicked,
}

impl Default for MyApp {
    fn default() -> Self {
        let code = r#"fn main() {
    println!("Hello, world!");
}
"#;

        Self {
            editor: CodeEditor::new(code, "rust"),
            input_value: String::new(),
        }
    }
}

impl MyApp {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditorEvent(event) => {
                self.editor.update(&event).map(Message::EditorEvent)
            }
            Message::InputChanged(value) => {
                self.input_value = value;
                self.editor.lose_focus();
                Task::none()
            }
            Message::TextInputClicked => {
                self.editor.lose_focus();
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let input = mouse_area(
            text_input("Type something...", &self.input_value)
                .on_input(Message::InputChanged)
                .padding(8),
        )
        .on_press(Message::TextInputClicked);

        container(
            column![input, self.editor.view().map(Message::EditorEvent)]
                .spacing(10)
                .height(iced::Fill),
        )
        .padding(20)
        .into()
    }
}

fn main() -> iced::Result {
    iced::run(MyApp::update, MyApp::view)
}
