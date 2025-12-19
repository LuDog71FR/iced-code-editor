# Iced Code Editor

A high-performance code editor widget for Iced.

This crate provides a canvas-based code editor with syntax highlighting,
line numbers, and text selection capabilities for the Iced GUI framework.

## Features

- **Syntax highlighting** for multiple programming languages
- **Line numbers** with styled gutter
- **Text selection** via mouse drag and keyboard
- **Clipboard operations** (copy, paste)
- **Undo/Redo** with smart command grouping and configurable history
- **Custom scrollbars** with themed styling
- **Focus management** for multiple editors
- **Dark & light themes** support with customizable colors
- **Undo/Redo** with command history

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

| Shortcut           | Action                         |
| ------------------ | ------------------------------ |
| **Backspace**      | Delete character before cursor |
| **Delete**         | Delete character after cursor  |
| **Shift + Delete** | Delete selected text           |
| **Enter**          | Insert new line                |

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

## Themes

The editor supports both dark and light themes:

```rust
use iced_code_editor::{CodeEditor, theme};
// Create an editor with dark theme (default)
let mut editor = CodeEditor::new("fn main() {}", "rs");
// Switch to light theme
editor.set_theme(theme::light(&iced::Theme::Light));
```

## Supported Languages

The editor supports syntax highlighting through the `syntect` crate:

- Python (`"py"` or `"python"`)
- Lua (`"lua"`)
- Rust (`"rs"` or `"rust"`)
- JavaScript (`"js"` or `"javascript"`)
- And many more...

For a complete list, refer to the `syntect` crate documentation.
