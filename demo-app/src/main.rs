use iced::widget::{container, mouse_area, row};
use iced::{Element, Subscription, Task, window};
use iced_code_editor::CanvasEditor;
use iced_code_editor::CanvasEditorMessage;

/// Main entry point for the canvas-based editor demo.
fn main() -> iced::Result {
    iced::application(DemoApp::new, DemoApp::update, DemoApp::view)
        .subscription(DemoApp::subscription)
        .run()
}

/// Which editor is currently focused
#[derive(Debug, Clone, Copy, PartialEq)]
enum FocusedEditor {
    Python,
    Lua,
}

/// Demo application with canvas-based high-performance editors.
struct DemoApp {
    /// Python code editor instance
    python_editor: CanvasEditor,
    /// Lua code editor instance
    lua_editor: CanvasEditor,
    /// Which editor currently has focus
    focused: FocusedEditor,
}

/// Messages that can be sent within the application.
#[derive(Debug, Clone)]
enum Message {
    /// Event from the Python editor
    PythonEditorEvent(CanvasEditorMessage),
    /// Event from the Lua editor
    LuaEditorEvent(CanvasEditorMessage),
    /// Python editor was clicked (focus it)
    FocusPython,
    /// Lua editor was clicked (focus it)
    FocusLua,
    /// Periodic tick for cursor blinking
    Tick,
}

impl DemoApp {
    /// Creates a new demo app with example Python and Lua code.
    fn new() -> (Self, Task<Message>) {
        // Example Python code with enough lines to trigger scrollbar
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

def factorial(n):
    """Calculate factorial."""
    if n <= 1:
        return 1
    return n * factorial(n-1)

def bubble_sort(arr):
    """Simple bubble sort implementation."""
    n = len(arr)
    for i in range(n):
        for j in range(0, n-i-1):
            if arr[j] > arr[j+1]:
                arr[j], arr[j+1] = arr[j+1], arr[j]
    return arr

def binary_search(arr, target):
    """Binary search implementation."""
    left, right = 0, len(arr) - 1
    
    while left <= right:
        mid = (left + right) // 2
        if arr[mid] == target:
            return mid
        elif arr[mid] < target:
            left = mid + 1
        else:
            right = mid - 1
    return -1

class Person:
    """Example class."""
    def __init__(self, name, age):
        self.name = name
        self.age = age
    
    def greet(self):
        print(f"Hello, I'm {self.name} and I'm {self.age} years old")
    
    def birthday(self):
        self.age += 1
        print(f"Happy birthday! Now {self.age} years old")

# Main execution
if __name__ == "__main__":
    hello_world()
    print(f"Fibonacci(10) = {fibonacci(10)}")
    print(f"Factorial(5) = {factorial(5)}")
    
    numbers = [64, 34, 25, 12, 22, 11, 90]
    print(f"Sorted: {bubble_sort(numbers)}")
    
    person = Person("Alice", 30)
    person.greet()
    person.birthday()
"#;

        // Example Lua code with enough lines to trigger scrollbar
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

function factorial(n)
    if n <= 1 then
        return 1
    end
    return n * factorial(n - 1)
end

function bubble_sort(arr)
    local n = #arr
    for i = 1, n do
        for j = 1, n - i do
            if arr[j] > arr[j + 1] then
                arr[j], arr[j + 1] = arr[j + 1], arr[j]
            end
        end
    end
    return arr
end

function binary_search(arr, target)
    local left, right = 1, #arr
    
    while left <= right do
        local mid = math.floor((left + right) / 2)
        if arr[mid] == target then
            return mid
        elseif arr[mid] < target then
            left = mid + 1
        else
            right = mid - 1
        end
    end
    return -1
end

-- Tables (dictionaries)
local person = {
    name = "John",
    age = 30,
    greet = function(self)
        print("Hello, I'm " .. self.name)
    end,
    birthday = function(self)
        self.age = self.age + 1
        print("Happy birthday! Now " .. self.age .. " years old")
    end
}

-- Main execution
hello_world()
print("Fibonacci(10) = " .. fibonacci(10))
print("Factorial(5) = " .. factorial(5))

local numbers = {64, 34, 25, 12, 22, 11, 90}
bubble_sort(numbers)
print("Sorted array")

person:greet()
person:birthday()
"#;

        (
            Self {
                python_editor: CanvasEditor::new(python_content, "py"),
                lua_editor: CanvasEditor::new(lua_content, "lua"),
                focused: FocusedEditor::Python, // Python starts focused
            },
            Task::none(),
        )
    }

    /// Handles application messages and updates editor state.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PythonEditorEvent(event) => {
                // Only update Python editor if it's focused
                if self.focused == FocusedEditor::Python {
                    self.python_editor
                        .update(&event)
                        .map(Message::PythonEditorEvent)
                } else {
                    Task::none()
                }
            }
            Message::LuaEditorEvent(event) => {
                // Only update Lua editor if it's focused
                if self.focused == FocusedEditor::Lua {
                    self.lua_editor.update(&event).map(Message::LuaEditorEvent)
                } else {
                    Task::none()
                }
            }
            Message::FocusPython => {
                self.focused = FocusedEditor::Python;
                Task::none()
            }
            Message::FocusLua => {
                self.focused = FocusedEditor::Lua;
                Task::none()
            }
            Message::Tick => {
                // Send Tick to both editors for cursor blinking
                let python_task = self
                    .python_editor
                    .update(&CanvasEditorMessage::Tick)
                    .map(Message::PythonEditorEvent);
                let lua_task = self
                    .lua_editor
                    .update(&CanvasEditorMessage::Tick)
                    .map(Message::LuaEditorEvent);
                Task::batch([python_task, lua_task])
            }
        }
    }

    /// Subscription for periodic cursor blink updates.
    fn subscription(&self) -> Subscription<Message> {
        // Use window frames for periodic updates
        let _ = self; // Suppress unused self warning
        window::frames().map(|_| Message::Tick)
    }

    /// Renders the application view with two side-by-side editors.
    fn view(&self) -> Element<'_, Message> {
        let python_focused = self.focused == FocusedEditor::Python;
        let lua_focused = self.focused == FocusedEditor::Lua;

        // Python editor with focus indicator and click detection
        let python_view = mouse_area(
            container(self.python_editor.view().map(Message::PythonEditorEvent)).style(
                move |_theme| {
                    container::Style {
                        border: iced::Border {
                            color: if python_focused {
                                iced::Color::from_rgb(0.3, 0.6, 1.0) // Blue border when focused
                            } else {
                                iced::Color::from_rgb(0.2, 0.2, 0.2) // Gray border when not focused
                            },
                            width: 2.0,
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    }
                },
            ),
        )
        .on_press(Message::FocusPython);

        // Lua editor with focus indicator and click detection
        let lua_view = mouse_area(
            container(self.lua_editor.view().map(Message::LuaEditorEvent)).style(move |_theme| {
                container::Style {
                    border: iced::Border {
                        color: if lua_focused {
                            iced::Color::from_rgb(0.3, 0.6, 1.0) // Blue border when focused
                        } else {
                            iced::Color::from_rgb(0.2, 0.2, 0.2) // Gray border when not focused
                        },
                        width: 2.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }
            }),
        )
        .on_press(Message::FocusLua);

        container(row![python_view, lua_view].spacing(10))
            .padding(10)
            .center(iced::Fill)
            .into()
    }
}
