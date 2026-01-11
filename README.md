<div align="center">

![Logo](https://github.com/LuDog71FR/iced-code-editor/blob/main/logo.svg)

# Iced Code Editor

A high-performance, canvas-based code editor widget for [Iced](https://github.com/iced-rs/iced).

[![Crates.io](https://img.shields.io/crates/v/iced-code-editor.svg)](https://crates.io/crates/iced-code-editor)
[![Documentation](https://docs.rs/iced-code-editor/badge.svg)](https://docs.rs/iced-code-editor)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/LuDog71FR/iced-code-editor/blob/main/LICENSE)
[![Downloads](https://img.shields.io/crates/d/iced-code-editor.svg)](https://crates.io/crates/iced-code-editor)
[![Build Status](https://github.com/LuDog71FR/iced-code-editor/workflows/Rust/badge.svg)](https://github.com/LuDog71FR/iced-code-editor/actions)

</div>

## Overview

This crate provides a fully-featured code editor widget with syntax highlighting, line numbers, text selection, and comprehensive keyboard navigation for the Iced GUI framework.

![Demo App](https://github.com/LuDog71FR/iced-code-editor/blob/main/screenshot_demo_app.png)
![Demo Screenshot Dark Theme](https://github.com/LuDog71FR/iced-code-editor/blob/main/screenshot_dark_theme.png) ![Demo Screenshot Light Theme](https://github.com/LuDog71FR/iced-code-editor/blob/main/screenshot_light_theme.png)

## Features

- **Syntax highlighting** for multiple programming languages via [syntect](https://github.com/trishume/syntect)
- **Line numbers** with styled gutter
- **Text selection** via mouse drag and keyboard shortcuts
- **Clipboard operations** (copy, paste)
- **Undo/Redo** with smart command grouping and configurable history
- **Custom scrollbars** with themed styling
- **Focus management** for multiple editors
- **Native Iced theme support** - Automatically adapts to all 23+ built-in Iced themes
- **Line wrapping** to split long lines
- **High performance** canvas-based rendering
- **Search and replace** text

## Planned features

- [ ] Multiple cursors for simultaneous editing at multiple positions
- [ ] Collapse/expand blocks
- [ ] Indentation-based or syntax-aware
- [ ] Minimap
- [ ] Auto-completion

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
iced = "0.14"
iced-code-editor = "0.3"
```

### Basic Example

Here's a minimal example to integrate the code editor into your Iced application:

```rust
use iced::widget::container;
use iced::{Element, Task};
use iced_code_editor::{CodeEditor, Message as EditorMessage};

struct MyApp {
    editor: CodeEditor,
}

#[derive(Debug, Clone)]
enum Message {
    EditorEvent(EditorMessage),
}

impl Default for MyApp {
    fn default() -> Self {
        let code = r#"fn main() {
    println!("Hello, world!");
}
"#;

        Self { editor: CodeEditor::new(code, "rust") }
    }
}

impl MyApp {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::EditorEvent(event) => {
                self.editor.update(&event).map(Message::EditorEvent)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        container(self.editor.view().map(Message::EditorEvent))
            .padding(20)
            .into()
    }
}

fn main() -> iced::Result {
    iced::run(MyApp::update, MyApp::view)
}
```

## Keyboard Shortcuts

The editor supports a comprehensive set of keyboard shortcuts:

### Navigation

| Shortcut                               | Action                        |
| -------------------------------------- | ----------------------------- |
| **Arrow Keys** (Up, Down, Left, Right) | Move cursor                   |
| **Shift + Arrows**                     | Move cursor with selection    |
| **Home** / **End**                     | Jump to start/end of line     |
| **Shift + Home** / **Shift + End**     | Select to start/end of line   |
| **Ctrl + Home** / **Ctrl + End**       | Jump to start/end of document |
| **Page Up** / **Page Down**            | Scroll one page up/down       |

### Editing

| Shortcut           | Action                                                                   |
| ------------------ | ------------------------------------------------------------------------ |
| **Backspace**      | Delete character before cursor (or delete selection if text is selected) |
| **Delete**         | Delete character after cursor (or delete selection if text is selected)  |
| **Shift + Delete** | Delete selected text (same as Delete when selection exists)              |
| **Enter**          | Insert new line                                                          |

### Clipboard

| Shortcut                           | Action               |
| ---------------------------------- | -------------------- |
| **Ctrl + C** or **Ctrl + Insert**  | Copy selected text   |
| **Ctrl + V** or **Shift + Insert** | Paste from clipboard |

### Undo/Redo

| Shortcut     | Action                     |
| ------------ | -------------------------- |
| **Ctrl + Z** | Undo last operation        |
| **Ctrl + Y** | Redo last undone operation |

The editor features smart command grouping - consecutive typing is grouped into single undo operations, while navigation or deletion actions create separate undo points.

### Search and Replace

| Shortcut     | Action                      |
| ------------ | --------------------------- |
| **Ctrl + F** | Open search dialog          |
| **Ctrl + H** | Open search and replace dialog |

## Usage Examples

### Changing Themes

The editor automatically adapts to any Iced theme. All 23+ built-in Iced themes are supported:

```rust
use iced_code_editor::theme;

// Apply any built-in Iced theme
editor.set_theme(theme::from_iced_theme(&iced::Theme::TokyoNightStorm));
editor.set_theme(theme::from_iced_theme(&iced::Theme::Dracula));
editor.set_theme(theme::from_iced_theme(&iced::Theme::Nord));
editor.set_theme(theme::from_iced_theme(&iced::Theme::CatppuccinMocha));
editor.set_theme(theme::from_iced_theme(&iced::Theme::GruvboxDark));

// Or use any theme from Theme::ALL
for theme in iced::Theme::ALL {
    editor.set_theme(theme::from_iced_theme(theme));
}
```

### Getting and Setting Content

```rust
// Get current content
let content = editor.content();

// Check if content has been modified
if editor.is_modified() {
    println!("Editor has unsaved changes");
}

// Mark content as saved (e.g., after saving to file)
editor.mark_saved();
```

### Cursor Blinking

For cursor blinking animation, subscribe to window frames:

```rust
use iced::{Subscription, window};

fn subscription(&self) -> Subscription<Message> {
    window::frames().map(|_| Message::Tick)
}

// In your update function
Message::Tick => {
    self.editor.update(&EditorMessage::Tick).map(Message::EditorEvent)
}
```

## Themes

The editor natively supports all built-in Iced themes with automatic color adaptation.

Each theme automatically provides:

- Optimized background and foreground colors
- Adaptive gutter (line numbers) styling
- Appropriate text selection colors
- Themed cursor appearance
- Custom scrollbar styling
- Subtle current line highlighting

The editor intelligently adapts colors from the Iced theme palette for optimal code readability.

## Supported Languages

The editor supports syntax highlighting for numerous languages via the `syntect` crate:

- **Rust** (`"rs"` or `"rust"`)
- **Python** (`"py"` or `"python"`)
- **JavaScript/TypeScript** (`"js"`, `"javascript"`, `"ts"`, `"typescript"`)
- **Lua** (`"lua"`)
- **C/C++** (`"c"`, `"cpp"`, `"c++"`)
- **Java** (`"java"`)
- **Go** (`"go"`)
- **HTML/CSS** (`"html"`, `"css"`)
- **Markdown** (`"md"`, `"markdown"`)
- And many more...

For a complete list, refer to the [syntect documentation](https://docs.rs/syntect/).

## Demo Application

A full-featured demo application is included in the `demo-app` directory, showcasing:

- File operations (open, save, save as)
- Theme switching
- Modified state tracking
- Clipboard operations
- Full keyboard navigation

Run it with:

```bash
cargo run --package demo-app --release
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

Check [docs\DEV.md](https://github.com/LuDog71FR/iced-code-editor/blob/main/docs/DEV.md) for more details.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

- Built with [Iced](https://github.com/iced-rs/iced) - A cross-platform GUI library for Rust
- Syntax highlighting powered by [syntect](https://github.com/trishume/syntect)
