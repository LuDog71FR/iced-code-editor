//! Demo application for iced-code-editor with pane_grid layout.
//!
//! This demo reproduces a typical IDE layout with:
//! - A toolbar at the top
//! - A vertical pane_grid with:
//!   - Top pane: DropDown menu + CodeEditor (height constrained to 400px)
//!   - Bottom pane: Output/Log area
//!
//! Remarks:
//! This layout is designed to test overflow and z-index issues.

use iced::widget::{
    PaneGrid, Space, button, column, container, pane_grid, pick_list, row,
    scrollable, text,
};
use iced::{Color, Element, Length, Subscription, Task, window};
use iced_aw::widget::drop_down::DropDown;
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
    const ALL: [EditorTheme; 2] = [EditorTheme::Dark, EditorTheme::Light];
}

impl std::fmt::Display for EditorTheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EditorTheme::Dark => write!(f, "Dark"),
            EditorTheme::Light => write!(f, "Light"),
        }
    }
}

/// Code templates available in the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Template {
    Empty,
    HelloWorld,
    Fibonacci,
    Factorial,
}

impl Template {
    const ALL: [Template; 4] = [
        Template::Empty,
        Template::HelloWorld,
        Template::Fibonacci,
        Template::Factorial,
    ];

    fn name(&self) -> &'static str {
        match self {
            Template::Empty => "Empty",
            Template::HelloWorld => "Hello World",
            Template::Fibonacci => "Fibonacci",
            Template::Factorial => "Factorial",
        }
    }

    fn content(&self) -> &'static str {
        match self {
            Template::Empty => "",
            Template::HelloWorld => {
                r#"-- Hello World in Lua
print("Hello, World!")
"#
            }
            Template::Fibonacci => {
                r#"-- Fibonacci sequence in Lua
function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

-- Print first 10 Fibonacci numbers
for i = 0, 10 do
    print("fib(" .. i .. ") = " .. fibonacci(i))
end
"#
            }
            Template::Factorial => {
                r#"-- Factorial function in Lua
function factorial(n)
    if n <= 1 then
        return 1
    end
    return n * factorial(n - 1)
end

-- Calculate factorials
for i = 1, 10 do
    print(i .. "! = " .. factorial(i))
end
"#
            }
        }
    }
}

/// Pane content types.
#[derive(Debug, Clone, Copy)]
enum PaneType {
    Editor,
    Output,
}

/// Demo application state.
struct DemoApp {
    /// Code editor
    editor: CodeEditor,
    /// Current file path
    current_file: Option<PathBuf>,
    /// Error message
    error_message: Option<String>,
    /// Current theme
    current_theme: EditorTheme,
    /// Background color
    background_color: Color,
    /// Pane grid state
    panes: pane_grid::State<PaneType>,
    /// Dropdown expanded state
    dropdown_expanded: bool,
    /// Log messages for output pane
    log_messages: Vec<String>,
}

/// Application messages.
#[derive(Debug, Clone)]
enum Message {
    /// Editor event
    EditorEvent(EditorMessage),
    /// Open file
    OpenFile,
    /// File opened
    FileOpened(Result<(PathBuf, String), String>),
    /// Save file
    SaveFile,
    /// Save file as
    SaveFileAs,
    /// File saved
    FileSaved(Result<PathBuf, String>),
    /// Cursor blink tick
    Tick,
    /// Theme changed
    ThemeChanged(EditorTheme),
    /// Pane resized
    PaneResized(pane_grid::ResizeEvent),
    /// Toggle dropdown
    DropdownToggle,
    /// Template selected
    TemplateSelected(Template),
    /// Clear log
    ClearLog,
    /// Run code (simulated)
    RunCode,
}

impl DemoApp {
    /// Creates a new instance of the application.
    fn new() -> (Self, Task<Message>) {
        let default_content = r#"-- Lua code editor demo
-- This demo tests pane_grid layout with CodeEditor

function greet(name)
    print("Hello, " .. name .. "!")
end

greet("World")
"#;

        // Create vertical pane_grid with editor on top, output on bottom
        let (mut panes, editor_pane) = pane_grid::State::new(PaneType::Editor);
        // Split always succeeds when called on a valid pane
        if let Some((_output_pane, _split)) = panes.split(
            pane_grid::Axis::Horizontal,
            editor_pane,
            PaneType::Output,
        ) {
            // Split succeeded, panes is now configured
        }

        let log_messages = vec![
            "[INFO] Application started".to_string(),
            "[INFO] Editor initialized with default content".to_string(),
        ];

        (
            Self {
                editor: CodeEditor::new(default_content, "lua"),
                current_file: None,
                error_message: None,
                current_theme: EditorTheme::Dark,
                background_color: Color::from_rgb(0.15, 0.15, 0.15),
                panes,
                dropdown_expanded: false,
                log_messages,
            },
            Task::none(),
        )
    }

    /// Adds a log message.
    fn log(&mut self, level: &str, message: &str) {
        self.log_messages.push(format!("[{}] {}", level, message));
    }

    /// Handles messages and updates the application state.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditorEvent(event) => {
                self.editor.update(&event).map(Message::EditorEvent)
            }
            Message::OpenFile => {
                self.log("INFO", "Opening file dialog...");
                Task::perform(open_file_dialog(), Message::FileOpened)
            }
            Message::FileOpened(result) => match result {
                Ok((path, content)) => {
                    self.log("INFO", &format!("Opened: {}", path.display()));
                    let task = self.editor.reset(&content);
                    let style = match self.current_theme {
                        EditorTheme::Dark => theme::dark(&iced::Theme::Dark),
                        EditorTheme::Light => theme::light(&iced::Theme::Light),
                    };
                    self.editor.set_theme(style);
                    self.editor.mark_saved();
                    self.current_file = Some(path);
                    self.error_message = None;
                    task.map(Message::EditorEvent)
                }
                Err(err) => {
                    self.log("ERROR", &err);
                    self.error_message = Some(err);
                    Task::none()
                }
            },
            Message::SaveFile => {
                if let Some(path) = self.current_file.clone() {
                    self.log("INFO", &format!("Saving to: {}", path.display()));
                    let content = self.editor.content();
                    Task::perform(save_file(path, content), Message::FileSaved)
                } else {
                    self.update(Message::SaveFileAs)
                }
            }
            Message::SaveFileAs => {
                self.log("INFO", "Opening save dialog...");
                let content = self.editor.content();
                Task::perform(save_file_as_dialog(content), Message::FileSaved)
            }
            Message::FileSaved(result) => {
                match result {
                    Ok(path) => {
                        self.log("INFO", &format!("Saved: {}", path.display()));
                        self.current_file = Some(path);
                        self.editor.mark_saved();
                        self.error_message = None;
                    }
                    Err(err) => {
                        self.log("ERROR", &err);
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
                self.log("INFO", &format!("Theme changed to: {}", new_theme));
                self.current_theme = new_theme;
                let style = match new_theme {
                    EditorTheme::Dark => theme::dark(&iced::Theme::Dark),
                    EditorTheme::Light => theme::light(&iced::Theme::Light),
                };
                self.editor.set_theme(style);
                self.background_color = match new_theme {
                    EditorTheme::Dark => Color::from_rgb(0.15, 0.15, 0.15),
                    EditorTheme::Light => Color::from_rgb(0.92, 0.92, 0.92),
                };
                Task::none()
            }
            Message::PaneResized(pane_grid::ResizeEvent { split, ratio }) => {
                self.panes.resize(split, ratio);
                Task::none()
            }
            Message::DropdownToggle => {
                self.dropdown_expanded = !self.dropdown_expanded;
                Task::none()
            }
            Message::TemplateSelected(template) => {
                self.log(
                    "INFO",
                    &format!("Template selected: {}", template.name()),
                );
                let task = self.editor.reset(template.content());
                let style = match self.current_theme {
                    EditorTheme::Dark => theme::dark(&iced::Theme::Dark),
                    EditorTheme::Light => theme::light(&iced::Theme::Light),
                };
                self.editor.set_theme(style);
                self.dropdown_expanded = false;
                self.current_file = None;
                task.map(Message::EditorEvent)
            }
            Message::ClearLog => {
                self.log_messages.clear();
                self.log("INFO", "Log cleared");
                Task::none()
            }
            Message::RunCode => {
                self.log("INFO", "Running code... (simulated)");
                let line_count = self.editor.content().lines().count();
                self.log("OUTPUT", &format!("Script has {} lines", line_count));
                self.log("OUTPUT", "Execution completed (simulated)");
                Task::none()
            }
        }
    }

    /// Subscription for periodic updates.
    fn subscription(&self) -> Subscription<Message> {
        let _ = self;
        window::frames().map(|_| Message::Tick)
    }

    /// Renders the user interface.
    fn view(&self) -> Element<'_, Message> {
        let text_color = match self.current_theme {
            EditorTheme::Dark => Color::from_rgb(0.9, 0.9, 0.9),
            EditorTheme::Light => Color::from_rgb(0.1, 0.1, 0.1),
        };

        // Toolbar
        let toolbar = row![
            button(text("Open")).on_press(Message::OpenFile),
            button(text("Save")).on_press(Message::SaveFile),
            button(text("Save As")).on_press(Message::SaveFileAs),
            button(text("Run")).on_press(Message::RunCode),
            text(self.file_status())
                .style(move |_| text::Style { color: Some(text_color) }),
            Space::new().width(Length::Fill),
            text("Theme:")
                .style(move |_| text::Style { color: Some(text_color) }),
            pick_list(
                &EditorTheme::ALL[..],
                Some(self.current_theme),
                Message::ThemeChanged
            ),
        ]
        .spacing(10)
        .padding(10)
        .align_y(iced::Center);

        // Error message if any
        let error_bar = if let Some(err) = &self.error_message {
            container(text(format!("Error: {}", err)).style(|_| text::Style {
                color: Some(Color::from_rgb(1.0, 0.3, 0.3)),
            }))
            .padding(5)
            .width(Length::Fill)
        } else {
            container(text("")).height(0)
        };

        // Pane grid
        let pane_grid =
            PaneGrid::new(&self.panes, |_id, pane, _is_maximized| {
                let title_bar_style = match self.current_theme {
                    EditorTheme::Dark => Color::from_rgb(0.2, 0.2, 0.25),
                    EditorTheme::Light => Color::from_rgb(0.85, 0.85, 0.88),
                };

                match pane {
                    PaneType::Editor => {
                        let title = pane_grid::TitleBar::new(
                            text("Editor").style(move |_| text::Style {
                                color: Some(text_color),
                            }),
                        )
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(
                                title_bar_style,
                            )),
                            ..Default::default()
                        })
                        .padding(5);

                        pane_grid::Content::new(
                            self.view_editor_pane(text_color),
                        )
                        .title_bar(title)
                        .style(|_| container::Style {
                            background: Some(iced::Background::Color(
                                self.background_color,
                            )),
                            ..Default::default()
                        })
                    }
                    PaneType::Output => {
                        let title = pane_grid::TitleBar::new(
                            text("Output").style(move |_| text::Style {
                                color: Some(text_color),
                            }),
                        )
                        .style(move |_| container::Style {
                            background: Some(iced::Background::Color(
                                title_bar_style,
                            )),
                            ..Default::default()
                        })
                        .padding(5);

                        pane_grid::Content::new(
                            self.view_output_pane(text_color),
                        )
                        .title_bar(title)
                        .style(|_| container::Style {
                            background: Some(iced::Background::Color(
                                self.background_color,
                            )),
                            ..Default::default()
                        })
                    }
                }
            })
            .on_resize(10, Message::PaneResized)
            .spacing(2);

        // Main layout
        container(
            column![toolbar, error_bar, pane_grid]
                .spacing(0)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .style(move |_| container::Style {
            background: Some(iced::Background::Color(self.background_color)),
            ..Default::default()
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    /// Renders the editor pane content.
    fn view_editor_pane(&self, _text_color: Color) -> Element<'_, Message> {
        // Dropdown button
        let dropdown_text =
            if self.dropdown_expanded { "Templates ^" } else { "Templates v" };

        let dropdown_button = button(text(dropdown_text).size(14))
            .on_press(Message::DropdownToggle)
            .padding(8);

        // Dropdown overlay content
        let template_buttons: Vec<Element<'_, Message>> = Template::ALL
            .iter()
            .map(|template| {
                button(text(template.name()).size(14).width(Length::Fill))
                    .on_press(Message::TemplateSelected(*template))
                    .width(Length::Fill)
                    .padding(8)
                    .style(|theme: &iced::Theme, status| {
                        let palette = theme.extended_palette();
                        match status {
                            iced::widget::button::Status::Hovered => {
                                iced::widget::button::Style {
                                    background: Some(iced::Background::Color(
                                        palette.primary.weak.color,
                                    )),
                                    text_color: palette.primary.weak.text,
                                    ..Default::default()
                                }
                            }
                            _ => iced::widget::button::Style {
                                background: Some(iced::Background::Color(
                                    palette.background.base.color,
                                )),
                                text_color: palette.background.base.text,
                                ..Default::default()
                            },
                        }
                    })
                    .into()
            })
            .collect();

        let dropdown_overlay =
            container(column(template_buttons).spacing(0).width(Length::Fill))
                .width(Length::Fixed(200.0))
                .style(move |_| container::Style {
                    background: Some(iced::Background::Color(Color::from_rgb(
                        0.2, 0.2, 0.25,
                    ))),
                    border: iced::Border {
                        color: Color::from_rgb(0.3, 0.3, 0.35),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                });

        // DropDown widget from iced_aw
        let dropdown = DropDown::new(
            dropdown_button,
            dropdown_overlay,
            self.dropdown_expanded,
        )
        .on_dismiss(Message::DropdownToggle);

        // Editor in a constrained container (400px height, clipped)
        let editor_view =
            container(self.editor.view().map(Message::EditorEvent))
                .width(Length::Fill)
                .clip(true)
                .style(|_| container::Style {
                    border: iced::Border {
                        color: Color::from_rgb(0.3, 0.3, 0.35),
                        width: 1.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                });

        container(
            column![row![dropdown].padding(10), editor_view,]
                .spacing(5)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .clip(true)
        .into()
    }

    /// Renders the output pane content.
    fn view_output_pane(&self, text_color: Color) -> Element<'_, Message> {
        // Clear button with hover effect
        let clear_button = button(text("Clear").size(12))
            .on_press(Message::ClearLog)
            .padding(4)
            .style(|theme: &iced::Theme, status| {
                let palette = theme.extended_palette();
                match status {
                    iced::widget::button::Status::Hovered => {
                        iced::widget::button::Style {
                            background: Some(iced::Background::Color(
                                palette.primary.weak.color,
                            )),
                            text_color: palette.primary.weak.text,
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    }
                    _ => iced::widget::button::Style {
                        background: Some(iced::Background::Color(
                            palette.background.weak.color,
                        )),
                        text_color: palette.background.weak.text,
                        border: iced::Border {
                            radius: 4.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                }
            });

        // Toolbar row with Clear button aligned to the right
        let toolbar = row![Space::new().width(Length::Fill), clear_button]
            .padding(5)
            .align_y(iced::Center);

        // Log messages content
        let log_content: Vec<Element<'_, Message>> = self
            .log_messages
            .iter()
            .map(|msg| {
                let color = if msg.contains("[ERROR]") {
                    Color::from_rgb(1.0, 0.4, 0.4)
                } else if msg.contains("[OUTPUT]") {
                    Color::from_rgb(0.4, 1.0, 0.4)
                } else {
                    text_color
                };

                text(msg)
                    .size(13)
                    .style(move |_| text::Style { color: Some(color) })
                    .into()
            })
            .collect();

        let log_scrollable = scrollable(
            column(log_content).spacing(2).padding(10).width(Length::Fill),
        )
        .height(Length::Fill)
        .width(Length::Fill);

        column![toolbar, log_scrollable]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Returns the file status string.
    fn file_status(&self) -> String {
        let file_name = self
            .current_file
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("New file");

        let modified = if self.editor.is_modified() { " *" } else { "" };
        format!("{}{}", file_name, modified)
    }
}

/// Opens a file dialog.
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

/// Saves content to a file.
async fn save_file(path: PathBuf, content: String) -> Result<PathBuf, String> {
    std::fs::write(&path, content)
        .map_err(|e| format!("Unable to write file: {}", e))?;
    Ok(path)
}

/// Opens a save-as dialog.
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
