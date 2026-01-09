# Development Documentation

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
   - [High-Level Structure](#high-level-structure)
   - [Core Components](#core-components)
3. [Design Patterns](#design-patterns)
   - [Command Pattern (Undo/Redo)](#1-command-pattern-undoredo)
   - [Elm Architecture (Message-Update-View)](#2-elm-architecture-message-update-view)
   - [Module Separation by Concern](#3-module-separation-by-concern)
   - [Canvas-Based Rendering](#4-canvas-based-rendering)
   - [Interior Mutability for History](#5-interior-mutability-for-history)
4. [Key Implementation Details](#key-implementation-details)
   - [Syntax Highlighting](#syntax-highlighting)
   - [Virtual Scrolling](#virtual-scrolling)
   - [Cursor Blinking](#cursor-blinking)
   - [Selection Rendering](#selection-rendering)
   - [Scroll-to-Cursor](#scroll-to-cursor)
5. [Performance Considerations](#performance-considerations)
   - [Canvas Caching](#1-canvas-caching)
   - [Syntax Highlighting Optimization](#2-syntax-highlighting-optimization)
   - [Text Buffer Performance](#3-text-buffer-performance)
   - [Memory Usage](#4-memory-usage)
6. [Testing Strategy](#testing-strategy)
   - [Unit Tests](#unit-tests)
   - [Integration Tests](#integration-tests)
   - [Running Tests](#running-tests)
7. [Common Pitfalls](#common-pitfalls)
   - [UTF-8 Character Boundaries](#1-utf-8-character-boundaries)
   - [Cache Invalidation](#2-cache-invalidation)
   - [Command History Grouping](#3-command-history-grouping)
   - [Selection Direction](#4-selection-direction)
8. [Future Enhancements](#future-enhancements)
   - [Planned Features](#planned-features)
   - [Performance Improvements](#performance-improvements)
9. [Contributing Guidelines](#contributing-guidelines)
   - [Code Style](#code-style)
   - [Pull Request Process](#pull-request-process)
   - [Commit Messages](#commit-messages)
   - [Documentation](#documentation)
10. [Resources](#resources)
    - [Iced Framework](#iced-framework)
    - [Syntax Highlighting](#syntax-highlighting-1)
    - [Design Patterns](#design-patterns-1)
    - [Text Editor Algorithms](#text-editor-algorithms)
11. [License](#license)

## Overview

This document describes the architecture, design patterns, and implementation details of the `iced-code-editor` widget. It is intended for developers who want to understand how the widget works internally, contribute to the project, or extend its functionality.

## Architecture

### High-Level Structure

The widget follows a modular architecture with clear separation of concerns:

```
iced-code-editor/
├── lib.rs                    # Public API and documentation
├── text_buffer.rs            # Text storage and manipulation
├── theme.rs                  # Styling and theming system
└── canvas_editor/            # Core editor implementation
    ├── mod.rs                # Main editor struct and constants
    ├── canvas_impl.rs        # Canvas rendering (Iced Canvas trait)
    ├── clipboard.rs          # Clipboard operations
    ├── command.rs            # Command pattern for undo/redo
    ├── cursor.rs             # Cursor movement logic
    ├── history.rs            # Command history management
    ├── selection.rs          # Text selection logic
    ├── update.rs             # Message handling (Elm Architecture)
    └── view.rs               # UI view construction
```

### Core Components

#### 1. **CodeEditor** (`canvas_editor/mod.rs`)

The main widget struct that holds all editor state:

```rust
pub struct CodeEditor {
    buffer: TextBuffer,              // Text content
    cursor: (usize, usize),          // Cursor position (line, col)
    scroll_offset: f32,              // Vertical scroll position
    style: Style,                    // Visual theme
    syntax: String,                  // Language for highlighting
    selection_start: Option<...>,   // Selection anchors
    history: CommandHistory,         // Undo/redo system
    cache: canvas::Cache,            // Rendering optimization
    // ... other fields
}
```

**Key characteristics:**

- Single source of truth for editor state
- No external dependencies on text buffer format
- All state transitions happen through message handling

#### 2. **TextBuffer** (`text_buffer.rs`)

A line-based text storage optimized for editor operations:

```rust
pub struct TextBuffer {
    lines: Vec<String>,  // Lines without newline characters
}
```

**Design decisions:**

- **Line-based storage**: Fast random access for virtual scrolling
- **No rope data structure**: Simple implementation, sufficient for typical code files
- **UTF-8 aware**: Proper handling of multi-byte characters
- **Trade-offs**: O(n) for large insertions, but O(1) for line access

**Operations:**

- `insert_char()` - Insert single character
- `insert_newline()` - Split line at position
- `delete_char()` - Delete before cursor (backspace)
- `delete_forward()` - Delete at cursor (delete key)

#### 3. **Theme System** (`theme.rs`)

A trait-based theming system following Iced's styling conventions with native support for all Iced themes:

```rust
pub trait Catalog {
    type Class<'a>;
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

pub struct Style {
    background: Color,
    text_color: Color,
    gutter_background: Color,
    line_number_color: Color,
    current_line_highlight: Color,
    // ... other colors
}
```

**Features:**

- Implements Iced's `Catalog` trait for seamless integration
- Function-based styling (`StyleFn`) for dynamic themes
- **Native support for all 23+ Iced themes** via `from_iced_theme()`
- Automatic color adaptation based on light/dark theme detection
- Intelligent color adjustments for optimal code readability

**Theme Adaptation:**
The `from_iced_theme()` function automatically extracts colors from any Iced theme's extended palette:

- **Background/Text**: Uses `palette.background.base` for primary colors
- **Gutter**: Uses `palette.background.weak` for subtle distinction
- **Line Numbers**: Intelligently dimmed/blended based on theme darkness
- **Current Line**: Subtle highlight using `palette.primary.weak` with transparency
- **Scrollbar**: Uses `palette.secondary.weak` for visibility

**Color Helpers:**

- `darken()` / `lighten()` - Adjust color brightness
- `dim_color()` - Reduce intensity for dark themes
- `blend_colors()` - Mix colors for light themes
- `with_alpha()` - Apply transparency

**Supported Themes:**
All native Iced themes are automatically supported:

- Basic: Light, Dark
- Popular: Dracula, Nord, Solarized, Gruvbox
- Catppuccin: Latte, Frappé, Macchiato, Mocha
- Tokyo Night: TokyoNight, TokyoNightStorm (default), TokyoNightLight
- Kanagawa: Wave, Dragon, Lotus
- Others: Moonfly, Nightfly, Oxocarbon, Ferra

## Design Patterns

### 1. Command Pattern (Undo/Redo)

**Location:** `canvas_editor/command.rs`, `canvas_editor/history.rs`

The undo/redo system uses the Command pattern to make all text modifications reversible.

```rust
pub trait Command: Send + std::fmt::Debug {
    fn execute(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize));
    fn undo(&mut self, buffer: &mut TextBuffer, cursor: &mut (usize, usize));
}
```

**Command types:**

- `InsertCharCommand` - Single character insertion
- `DeleteCharCommand` - Backspace operation
- `DeleteForwardCommand` - Delete key operation
- `InsertNewlineCommand` - Enter key
- `InsertTextCommand` - Multi-character paste
- `DeleteRangeCommand` - Selection deletion
- `CompositeCommand` - Groups multiple commands

**Smart grouping:**

```rust
// Consecutive typing is grouped into one undo operation
history.begin_group("Typing");
// ... multiple InsertCharCommand ...
history.end_group();  // Now undoable as single operation
```

**Benefits:**

- Complete undo/redo support
- Command grouping for natural undo boundaries
- Save point tracking for modified state detection
- Configurable history size for memory management

### 2. Elm Architecture (Message-Update-View)

**Location:** `canvas_editor/update.rs`, `canvas_editor/view.rs`

The widget follows Iced's Elm-inspired architecture:

```rust
// View: Pure function that renders current state
pub fn view(&self) -> Element<'_, Message> { ... }

// Update: Pure function that processes messages
pub fn update(&mut self, message: &Message) -> Task<Message> { ... }

// Messages: All possible user interactions
pub enum Message {
    CharacterInput(char),
    ArrowKey(ArrowDirection, bool),
    Copy, Paste(String),
    Undo, Redo,
    // ...
}
```

**Benefits:**

- Predictable state management
- Easy to test (pure functions)
- Clear data flow
- Natural integration with Iced framework

### 3. Module Separation by Concern

Each module has a single, well-defined responsibility:

- **`cursor.rs`** - Cursor movement, scrolling, page up/down
- **`selection.rs`** - Text selection logic and range calculations
- **`clipboard.rs`** - Copy/paste operations
- **`canvas_impl.rs`** - Low-level Canvas drawing
- **`update.rs`** - Message routing and state transitions

This follows the **Single Responsibility Principle** and makes the codebase maintainable.

### 4. Canvas-Based Rendering

**Location:** `canvas_editor/canvas_impl.rs`

Instead of using Iced's high-level text widgets, we use the Canvas API for maximum performance:

```rust
impl canvas::Program<Message> for CodeEditor {
    fn draw(&self, ...) -> Vec<canvas::Geometry> {
        // Direct rendering of text, line numbers, selection
    }
}
```

**Why Canvas?**

- **Performance**: No widget tree overhead for large files
- **Control**: Pixel-perfect rendering of editor elements
- **Syntax highlighting**: Direct integration with syntect
- **Custom scrolling**: Fine-grained control over viewport

**Cache optimization:**

```rust
self.cache.clear();  // Invalidate on changes
// Canvas automatically caches unchanged frames
```

### 5. Interior Mutability for History

**Location:** `canvas_editor/history.rs`

The `CommandHistory` uses `Arc<Mutex<>>` for interior mutability:

```rust
pub struct CommandHistory {
    inner: Arc<Mutex<HistoryInner>>,
}
```

**Why?**

- Allows immutable borrows of `CodeEditor` while mutating history
- Thread-safe design (though used single-threaded in GUI)
- Enables cloning of `CommandHistory` without cloning commands

**Note:** This is safe because Iced is single-threaded. The mutex provides interior mutability, not actual concurrency.

## Key Implementation Details

### Syntax Highlighting

**Integration:** Uses `syntect` crate for syntax highlighting

```rust
// In canvas_impl.rs
let syntax = syntax_set.find_syntax_by_extension(&self.syntax);
let highlighter = HighlightLines::new(syntax, &theme_set.themes["base16-ocean.dark"]);

for line in visible_lines {
    let regions = highlighter.highlight_line(line, &syntax_set)?;
    for (style, text) in regions {
        // Draw text with style.foreground color
    }
}
```

**Optimizations:**

- Only highlight visible lines
- Cache highlighted regions (future enhancement)
- Lazy loading of syntax definitions

### Virtual Scrolling

Only visible lines are rendered:

```rust
let first_visible_line = (viewport_scroll / LINE_HEIGHT) as usize;
let visible_lines = (viewport_height / LINE_HEIGHT).ceil() as usize + 2; // +2 for buffer
let last_visible_line = (first_visible_line + visible_lines).min(line_count);

for line_idx in first_visible_line..last_visible_line {
    // Render only visible lines
}
```

**Benefits:**

- Constant rendering cost regardless of file size
- Smooth scrolling even for large files
- Memory efficient

### Cursor Blinking

**Implementation:** Frame-based animation via subscription

```rust
// In demo app
fn subscription(&self) -> Subscription<Message> {
    window::frames().map(|_| Message::Tick)
}

// In update()
Message::Tick => {
    if self.last_blink.elapsed() >= CURSOR_BLINK_INTERVAL {
        self.cursor_visible = !self.cursor_visible;
        self.cache.clear();  // Force redraw
    }
}
```

**Interval:** 530ms (standard cursor blink rate)

### Selection Rendering

**Normalization:** Selections are normalized before rendering

```rust
fn get_selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
    let (start, end) = (self.selection_start?, self.selection_end?);

    // Ensure start comes before end
    if start.0 < end.0 || (start.0 == end.0 && start.1 < end.1) {
        Some((start, end))
    } else {
        Some((end, start))  // Swap if reversed
    }
}
```

**Rendering:**

- Single-line: Simple rectangle
- Multi-line: Three rectangles (first line, middle lines, last line)

### Scroll-to-Cursor

**Auto-scrolling:** Cursor always stays visible

```rust
pub fn scroll_to_cursor(&self) -> Task<Message> {
    let cursor_y = self.cursor.0 as f32 * LINE_HEIGHT;

    if cursor_y < viewport_top + margin {
        // Scroll up
    } else if cursor_y > viewport_bottom - margin {
        // Scroll down
    }

    scroll_to(self.scrollable_id.clone(), AbsoluteOffset { y: new_scroll })
}
```

**Smart margins:** 2 lines of padding to prevent cursor at edge

## Performance Considerations

### 1. Canvas Caching

```rust
self.cache = canvas::Cache::default();  // Create cache
self.cache.clear();  // Invalidate on changes
```

Iced automatically caches canvas frames. We clear the cache only when content changes.

### 2. Syntax Highlighting Optimization

**Current:** Highlight all visible lines on every frame

**Future improvements:**

- Cache highlighted regions per line
- Incremental re-highlighting on edits
- Background parsing for large files

### 3. Text Buffer Performance

**Current limitations:**

- O(n) for inserting text in middle of line (string operations)
- O(1) for line access (vector indexing)

**Sufficient for:**

- Files up to ~10,000 lines
- Typical editing patterns (typing, deleting)

**Not optimal for:**

- Inserting/deleting large blocks in huge files
- Real-time collaborative editing

**Potential improvements:**

- Rope data structure for O(log n) operations
- Gap buffer for cursor-local edits
- Piece table for large file handling

### 4. Memory Usage

**Per editor instance:**

- Text buffer: ~1 byte per character + vector overhead
- Command history: Configurable (default 100 commands)
- Each command: ~80-200 bytes depending on type
- Canvas cache: ~memory of rendered frame

**Typical usage:**

- 1000-line file: ~50KB text + ~10KB history = ~60KB
- Very manageable for modern systems

## Testing Strategy

### Unit Tests

Each module has comprehensive unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() { ... }
}
```

**Coverage:**

- `text_buffer.rs`: All buffer operations
- `command.rs`: All command types and undo/redo
- `cursor.rs`: Cursor movement edge cases
- `selection.rs`: Selection normalization and extraction
- `update.rs`: Message handling and state transitions
- `theme.rs`: All Iced themes, color adaptation, helper functions

### Integration Tests

The demo application serves as an integration test, covering:

- File loading/saving
- Theme switching
- Clipboard operations
- Full keyboard navigation

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_insert_char

# Run tests with coverage (requires tarpaulin)
cargo tarpaulin --out Html
```

## Common Pitfalls

### 1. UTF-8 Character Boundaries

**Problem:** Rust strings are UTF-8, so byte indices ≠ character indices

**Solution:** Use char-aware indexing

```rust
fn char_to_byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map_or(s.len(), |(idx, _)| idx)
}
```

### 2. Cache Invalidation

**Problem:** Forgetting to clear cache leads to stale rendering

**Solution:** Clear cache on every state change

```rust
self.cursor = new_position;
self.cache.clear();  // Don't forget!
```

### 3. Command History Grouping

**Problem:** Forgetting to end groups causes memory leaks

**Solution:** Always pair `begin_group()` with `end_group()`

```rust
// On navigation, deletion, etc.
if self.is_grouping {
    self.history.end_group();
    self.is_grouping = false;
}
```

### 4. Selection Direction

**Problem:** User can drag selection backwards

**Solution:** Always normalize selection ranges

```rust
let (start, end) = self.get_selection_range()?;
// start is guaranteed to be before end
```

## Future Enhancements

### Planned Features

1. **Line wrapping**

   - Soft wraps for long lines
   - Configurable wrap column

2. **Multiple cursors**

   - Simultaneous editing at multiple positions
   - Requires refactoring cursor from `(usize, usize)` to `Vec<(usize, usize)>`

3. **Code folding**

   - Collapse/expand blocks
   - Indentation-based or syntax-aware

4. **Minimap**

   - Overview of entire file
   - Clickable navigation

5. **Search and replace**

   - Regex support
   - Incremental search
   - Replace with undo support

6. **Auto-completion**
   - LSP integration
   - Context-aware suggestions

### Performance Improvements

1. **Rope data structure** for better large-file performance
2. **Incremental syntax highlighting** to avoid re-highlighting entire file
3. **Virtual scrolling for horizontal** axis (long lines)
4. **Web Worker for highlighting** (when targeting WASM)

## Contributing Guidelines

### Code Style

- Follow Rust 2024 edition conventions
- Use `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Maintain existing documentation style
- Add unit tests for new features

### Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/my-feature`)
3. Make your changes with tests
4. Run full test suite (`cargo test`)
5. Run linter (`cargo clippy`)
6. Format code (`cargo fmt`)
7. Commit with clear message
8. Push and create pull request

### Commit messages

Follow the [Conventional Commits](https://www.conventionalcommits.org/) specification.

**Format:** `<type>(<scope>): <description>`

Where `<scope>` is optional and can be the affected module (e.g., `api`, `models`, `scheduler`).

**Types:**

- `feat` - New feature (e.g., `feat(api): add endpoint for task scheduling`)
- `fix` - Bug fix (e.g., `fix(models): correct timezone handling in timestamps`)
- `docs` - Documentation only (e.g., `docs: update installation instructions`)
- `style` - Code style/formatting (e.g., `style: apply rustfmt changes`)
- `refactor` - Code refactoring (e.g., `refactor(tasks): extract common validation logic`)
- `perf` - Performance improvement (e.g., `perf(db): optimize query with index`)
- `test` - Add or modify tests (e.g., `test(models): add unit tests for User model`)
- `build` - Build system changes (e.g., `build: update sqlx to 0.7`)
- `ci` - CI configuration (e.g., `ci: add clippy check to workflow`)
- `chore` - Maintenance tasks (e.g., `chore: update dependencies`)

**Breaking changes:** Add `!` after type/scope (e.g., `feat!: rename API endpoint` or `feat(api)!: change response format`)

### Documentation

- Public API must have doc comments
- Complex algorithms need inline comments
- Update README.md for user-facing changes
- Update DEV.md for architectural changes

## Resources

### Iced Framework

- [Iced GitHub](https://github.com/iced-rs/iced)
- [Iced Documentation](https://docs.rs/iced/)
- [Canvas Example](https://github.com/iced-rs/iced/tree/master/examples/canvas)

### Syntax Highlighting

- [syntect](https://github.com/trishume/syntect)
- [Sublime Text Syntax Definitions](https://www.sublimetext.com/docs/syntax.html)

### Design Patterns

- [Command Pattern](https://refactoring.guru/design-patterns/command)
- [Elm Architecture](https://guide.elm-lang.org/architecture/)

### Text Editor Algorithms

- [Text Editor: Data Structures](https://www.averylaird.com/programming/the%20text%20editor/2017/09/30/the-piece-table/)
- [Rope Science](https://www.foonathan.net/2015/03/rope-science/)
- [VSCode Text Buffer](https://code.visualstudio.com/blogs/2018/03/23/text-buffer-reimplementation)

## License

This project is licensed under the MIT License - see the LICENSE file for details.
