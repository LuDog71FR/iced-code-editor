mod code_editor;

use code_editor::CodeEditorComponent;
use iced::widget::{container, row};
use iced::{Element, Task};

fn main() -> iced::Result {
    iced::run("Code Editor - Python & Lua", DemoApp::update, DemoApp::view)
}

struct DemoApp {
    python_editor: CodeEditorComponent,
    lua_editor: CodeEditorComponent,
}

impl Default for DemoApp {
    fn default() -> Self {
        // Code Python
        let python_content = r#"def hello_world():
    """A simple greeting function."""
    print("Hello, World!")
    
    for i in range(10):
        print(f"Count: {i}")

def fibonacci(n):
    """Calculate fibonacci number."""
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)

# Main execution
if __name__ == "__main__":
    hello_world()
    print(f"Fibonacci(10) = {fibonacci(10)}")
"#
        .to_string();

        // Code Lua
        let lua_content = r#"-- Lua example code
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

-- Tables (dictionnaires)
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
person:greet()
"#
        .to_string();

        Self {
            python_editor: CodeEditorComponent::new_with_language(python_content, "py"),
            lua_editor: CodeEditorComponent::new_with_language(lua_content, "lua"),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    PythonEditorEvent(code_editor::component::Event),
    LuaEditorEvent(code_editor::component::Event),
}

impl DemoApp {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PythonEditorEvent(event) => self
                .python_editor
                .update(event)
                .map(Message::PythonEditorEvent),
            Message::LuaEditorEvent(event) => {
                self.lua_editor.update(event).map(Message::LuaEditorEvent)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        use iced::Padding;
        use iced::widget::{column, text};

        // Créer l'éditeur Python avec son label
        let python_view = column![
            container(text("Python").size(16))
                .padding(Padding::from([5, 10]))
                .style(|_theme| {
                    container::Style {
                        background: Some(iced::Color::from_rgb(0.15, 0.15, 0.18).into()),
                        border: iced::Border {
                            color: iced::Color::from_rgb(0.3, 0.3, 0.35),
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }
                }),
            self.python_editor.view().map(Message::PythonEditorEvent)
        ]
        .spacing(0);

        // Créer l'éditeur Lua avec son label
        let lua_view = column![
            container(text("Lua").size(16))
                .padding(Padding::from([5, 10]))
                .style(|_theme| {
                    container::Style {
                        background: Some(iced::Color::from_rgb(0.15, 0.15, 0.18).into()),
                        border: iced::Border {
                            color: iced::Color::from_rgb(0.3, 0.3, 0.35),
                            width: 0.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }
                }),
            self.lua_editor.view().map(Message::LuaEditorEvent)
        ]
        .spacing(0);

        container(row![python_view, lua_view].spacing(0))
            .width(iced::Fill)
            .height(iced::Fill)
            .into()
    }
}
