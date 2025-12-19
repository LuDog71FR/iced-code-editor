use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Element, Subscription, Task, window};
use iced_code_editor::Message as EditorMessage;
use iced_code_editor::{CodeEditor, theme};
use std::path::PathBuf;

/// Main entry point for the demo application.
fn main() -> iced::Result {
    iced::application(DemoApp::new, DemoApp::update, DemoApp::view)
        .subscription(DemoApp::subscription)
        .run()
}

/// Available themes for the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorTheme {
    Dark,
    Light,
}

impl EditorTheme {
    /// Returns all available themes.
    const ALL: [EditorTheme; 2] = [EditorTheme::Dark, EditorTheme::Light];
}

impl std::fmt::Display for EditorTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorTheme::Dark => write!(f, "🌙 Dark"),
            EditorTheme::Light => write!(f, "☀️ Light"),
        }
    }
}

/// Demo application with Lua editor and file management.
struct DemoApp {
    /// Lua code editor
    editor: CodeEditor,
    /// Path of the currently open file
    current_file: Option<PathBuf>,
    /// Error message to display (if any)
    error_message: Option<String>,
    /// Current editor theme
    current_theme: EditorTheme,
}

/// Application messages.
#[derive(Debug, Clone)]
enum Message {
    /// Editor event
    EditorEvent(EditorMessage),
    /// Request to open a file
    OpenFile,
    /// File opened successfully
    FileOpened(Result<(PathBuf, String), String>),
    /// Request to save the current file
    SaveFile,
    /// Request to save as a new file
    SaveFileAs,
    /// File saved successfully
    FileSaved(Result<PathBuf, String>),
    /// Periodic tick for cursor blinking
    Tick,
    /// Theme changed
    ThemeChanged(EditorTheme),
}

impl DemoApp {
    /// Creates a new instance of the application.
    fn new() -> (Self, Task<Message>) {
        // Default Lua content
        let lua_content = r#"-- Lua code editor
-- Use the buttons to open and save files

function hello_world()
    print("Hello, World!")
    
    for i = 1, 10 do
        print("Count: " .. i)
    end
end

function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

function factorial(n)
    if n <= 1 then
        return 1
    end
    return n * factorial(n - 1)
end

-- Tables (dictionaries)
local person = {
    name = "John",
    age = 30,
    greet = function(self)
        print("Hello, I'm " .. self.name)
    end
}

-- Main execution
hello_world()
print("Fibonacci(10) = " .. fibonacci(10))
print("Factorial(5) = " .. factorial(5))
person:greet()
"#;

        (
            Self {
                editor: CodeEditor::new(lua_content, "lua"),
                current_file: None,
                error_message: None,
                current_theme: EditorTheme::Dark,
            },
            Task::none(),
        )
    }

    /// Handles messages and updates the application state.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditorEvent(event) => {
                self.editor.update(&event).map(Message::EditorEvent)
            }
            Message::OpenFile => {
                // Open file picker asynchronously
                Task::perform(open_file_dialog(), Message::FileOpened)
            }
            Message::FileOpened(result) => {
                match result {
                    Ok((path, content)) => {
                        self.editor = CodeEditor::new(&content, "lua");
                        // Apply current theme to the new editor
                        let style = match self.current_theme {
                            EditorTheme::Dark => {
                                theme::dark(&iced::Theme::Dark)
                            }
                            EditorTheme::Light => {
                                theme::light(&iced::Theme::Light)
                            }
                        };
                        self.editor.set_theme(style);
                        // Mark as saved since we just loaded the file
                        self.editor.mark_saved();
                        self.current_file = Some(path);
                        self.error_message = None;
                    }
                    Err(err) => {
                        self.error_message = Some(err);
                    }
                }
                Task::none()
            }
            Message::SaveFile => {
                if let Some(path) = &self.current_file {
                    // Save to current file
                    let content = self.editor.content();
                    let path_clone = path.clone();
                    Task::perform(
                        save_file(path_clone, content),
                        Message::FileSaved,
                    )
                } else {
                    // No current file, ask where to save
                    self.update(Message::SaveFileAs)
                }
            }
            Message::SaveFileAs => {
                // Open picker to choose where to save
                let content = self.editor.content();
                Task::perform(save_file_as_dialog(content), Message::FileSaved)
            }
            Message::FileSaved(result) => {
                match result {
                    Ok(path) => {
                        self.current_file = Some(path);
                        // Mark as saved
                        self.editor.mark_saved();
                        self.error_message = None;
                    }
                    Err(err) => {
                        self.error_message = Some(err);
                    }
                }
                Task::none()
            }
            Message::Tick => self
                .editor
                .update(&EditorMessage::Tick)
                .map(Message::EditorEvent),
            Message::ThemeChanged(new_theme) => {
                // Change editor theme
                self.current_theme = new_theme;
                let style = match new_theme {
                    EditorTheme::Dark => theme::dark(&iced::Theme::Dark),
                    EditorTheme::Light => theme::light(&iced::Theme::Light),
                };
                self.editor.set_theme(style);
                Task::none()
            }
        }
    }

    /// Subscription for periodic updates.
    fn subscription(&self) -> Subscription<Message> {
        let _ = self; // Required for trait signature
        window::frames().map(|_| Message::Tick)
    }

    /// Renders the user interface.
    fn view(&self) -> Element<'_, Message> {
        // Theme selector
        let theme_picker = pick_list(
            &EditorTheme::ALL[..],
            Some(self.current_theme),
            Message::ThemeChanged,
        );

        // Toolbar at the top
        let toolbar = row![
            button(text("📂 Open")).on_press(Message::OpenFile),
            button(text("💾 Save")).on_press(Message::SaveFile),
            button(text("💾 Save As...")).on_press(Message::SaveFileAs),
            text(self.file_status()),
            theme_picker,
        ]
        .spacing(10)
        .padding(10);

        // Error message (if present)
        let error_view = if let Some(err) = &self.error_message {
            container(text(format!("❌ Error: {}", err)).style(|_theme| {
                text::Style {
                    color: Some(iced::Color::from_rgb(1.0, 0.3, 0.3)),
                }
            }))
            .padding(10)
        } else {
            container(text(""))
        };

        // Main editor
        let editor_view = container(
            self.editor.view().map(Message::EditorEvent),
        )
        .style(|_theme| container::Style {
            border: iced::Border {
                color: iced::Color::from_rgb(0.2, 0.2, 0.2),
                width: 1.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        });

        // Main layout
        container(
            column![toolbar, error_view, editor_view]
                .spacing(0)
                .width(iced::Fill)
                .height(iced::Fill),
        )
        .padding(0)
        .center(iced::Fill)
        .into()
    }

    /// Returns the file status for display.
    fn file_status(&self) -> String {
        let file_name = self
            .current_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("New file");

        let modified = if self.editor.is_modified() { " *" } else { "" };

        format!("📄 {}{}", file_name, modified)
    }
}

/// Opens a dialog box to select a file to open.
async fn open_file_dialog() -> Result<(PathBuf, String), String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Lua Files", &["lua"])
        .add_filter("All Files", &["*"])
        .set_title("Open Lua File")
        .pick_file()
        .await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        let content = std::fs::read_to_string(&path)
            .map_err(|e| format!("Unable to read file: {}", e))?;
        Ok((path, content))
    } else {
        Err("No file selected".to_string())
    }
}

/// Saves the content to an existing file.
async fn save_file(path: PathBuf, content: String) -> Result<PathBuf, String> {
    std::fs::write(&path, content)
        .map_err(|e| format!("Unable to write file: {}", e))?;
    Ok(path)
}

/// Opens a dialog box to save with a new name.
async fn save_file_as_dialog(content: String) -> Result<PathBuf, String> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Lua Files", &["lua"])
        .set_title("Save As")
        .save_file()
        .await;

    if let Some(file) = file {
        let path = file.path().to_path_buf();
        std::fs::write(&path, content)
            .map_err(|e| format!("Unable to write file: {}", e))?;
        Ok(path)
    } else {
        Err("Save cancelled".to_string())
    }
}
