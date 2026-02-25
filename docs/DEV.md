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
   - [Focus Management](#focus-management)
   - [Selection Rendering](#selection-rendering)
   - [Scroll-to-Cursor](#scroll-to-cursor)
   - [Internationalization (i18n)](#internationalization-i18n)
   - [CJK and Asian Character Support](#cjk-and-asian-character-support)
5. [Performance Considerations](#performance-considerations)
   - [Canvas Caching](#1-canvas-caching)
   - [Syntax Highlighting Optimization](#2-syntax-highlighting-optimization)
   - [Text Buffer Performance](#3-text-buffer-performance)
   - [Memory Usage](#4-memory-usage)
   - [CJK Character Width Calculation](#5-cjk-character-width-calculation)
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
    content_cache: canvas::Cache,    // Content rendering optimization
    overlay_cache: canvas::Cache,    // Overlay rendering optimization
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
self.content_cache.clear();
self.overlay_cache.clear();
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
    // Only process blinking if editor has focus (optimization)
    if self.is_focused() && self.last_blink.elapsed() >= CURSOR_BLINK_INTERVAL {
        self.cursor_visible = !self.cursor_visible;
        self.last_blink = std::time::Instant::now();
        self.overlay_cache.clear();  // Force redraw
    }
}
```

**Interval:** 530ms (standard cursor blink rate)

**Focus integration:** Blinking only occurs for the focused editor, reducing CPU usage when multiple editors are present. See [Focus Management](#focus-management) for details.

### Focus Management

**Location:** `canvas_editor/mod.rs`, `canvas_editor/update.rs`, `canvas_editor/canvas_impl.rs`

When multiple `CodeEditor` instances exist, only one should receive keyboard input and display a cursor. The focus system uses global atomic counters for coordination.

**Architecture:**

```rust
// Unique ID per editor instance
static EDITOR_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

// ID of currently focused editor (0 = none)
static FOCUSED_EDITOR_ID: AtomicU64 = AtomicU64::new(0);

pub struct CodeEditor {
    editor_id: u64,  // Assigned at creation
    // ...
}
```

**API:**

```rust
// Check if this editor has focus
pub fn is_focused(&self) -> bool {
    FOCUSED_EDITOR_ID.load(Ordering::Relaxed) == self.editor_id
}

// Request focus programmatically
pub fn request_focus(&self) {
    FOCUSED_EDITOR_ID.store(self.editor_id, Ordering::Relaxed);
}
```

**Automatic focus capture:**

- Mouse clicks inside an editor automatically capture focus
- First editor created receives focus by default

**Keyboard event filtering:**

```rust
// Only process keyboard events if focused
let focused_id = FOCUSED_EDITOR_ID.load(Ordering::Relaxed);
if focused_id != self.editor_id {
    return None;  // Ignore event
}
```

**Visual feedback:**

- Cursor only visible when editor has focus: `if self.cursor_visible && self.is_focused() { ... }`
- Cursor blinking paused for unfocused editors (performance optimization)

**Design rationale:** Global `AtomicU64` provides thread-safe coordination without locking overhead or parameter threading. `Ordering::Relaxed` is sufficient for single-threaded GUI context.

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

## Internationalization (i18n)

**Location:** `i18n.rs`, `locales/*.yml`

The editor uses `rust-i18n` with YAML translation files for multi-language support.

**Architecture:**

```rust
pub enum Language {
    English, French, Spanish,
}

pub struct Translations {
    language: Language,
}

impl Translations {
    pub fn new(language: Language) -> Self {
        rust_i18n::set_locale(language.to_locale());
        Self { language }
    }

    pub fn search_placeholder(&self) -> String {
        rust_i18n::t!("search.placeholder", locale = self.language.to_locale())
            .into_owned()
    }
}
```

**Translation files** (`locales/en.yml`, `fr.yml`, `es.yml`, ...):

```yaml
search:
  placeholder: "Search..."
  close_tooltip: "Close search dialog (Esc)"
replace:
  placeholder: "Replace..."
settings:
  case_sensitive_label: "Case sensitive"
```

**Key design decisions:**

- **Global locale**: `rust_i18n::set_locale()` sets global locale, tracked per instance
- **Owned strings**: Returns `String` (not `&str`) - `rust_i18n::t!()` returns `Cow<'_, str>`, we call `.into_owned()` to avoid lifetime issues
- **Initialization**: `rust_i18n::i18n!("locales", fallback = "en")` macro called in `lib.rs`

**Adding a new language:**

1. Create `locales/de.yml` with translation keys
2. Add `German` to `Language` enum
3. Update `to_locale()` to return `"de"`
4. Add tests

**See also:** [docs/i18n.md](https://github.com/LuDog71FR/iced-code-editor/blob/main/docs/i18n.md) for detailed documentation.

### CJK and Asian Character Support

**Location:** `canvas_editor/mod.rs`, `canvas_editor/ime_requester.rs`, `canvas_editor/canvas_impl.rs`

CJK characters (Chinese, Japanese, Korean) are "wide" characters that occupy twice the width of ASCII/Latin characters in monospace fonts. The editor must handle mixed-width text correctly for accurate cursor positioning, text selection, and rendering.

**Architecture:**

The editor uses a dual-width measurement system combined with Unicode-aware character classification and full IME (Input Method Editor) support for Asian language input.

#### Character Width System

Two distinct character widths are maintained and dynamically calculated based on the current font:

```rust
pub struct CodeEditor {
    char_width: f32,       // Width of narrow characters (ASCII, Latin)
    full_char_width: f32,  // Width of wide characters (CJK)
    // ...
}
```

**Width calculation** (`canvas_editor/mod.rs:398-435`):

```rust
fn recalculate_char_dimensions(&mut self, renderer: &Renderer) {
    // Measure narrow character width using 'a'
    self.char_width = self.measure_single_char_width(renderer, 'a');
    
    // Measure wide character width using '汉' (Chinese character)
    self.full_char_width = self.measure_single_char_width(renderer, '\u{6c49}');
    
    // Fallback if measurements return infinite values
    if !self.char_width.is_finite() {
        self.char_width = self.font_size / 2.0;
    }
    if !self.full_char_width.is_finite() {
        self.full_char_width = self.font_size;
    }
}
```

**Key characteristics:**

- Widths are recalculated whenever font or font size changes
- Uses actual font metrics from Iced's text measurement API
- Fallback values ensure robustness (narrow = font_size/2, wide = font_size)

#### Unicode Width Detection

**Integration:** Uses `unicode_width` crate (implements Unicode Standard Annex #11 - East Asian Width)

The `measure_char_width()` function classifies characters and returns appropriate width (`canvas_editor/mod.rs:61-96`):

```rust
pub(crate) fn measure_char_width(
    c: char,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    use unicode_width::UnicodeWidthChar;
    
    match c.width() {
        Some(w) if w > 1 => full_char_width,  // Wide (CJK)
        Some(_) => char_width,                 // Narrow (ASCII/Latin)
        None => 0.0,                           // Control characters
    }
}
```

**Character classification:**

- **Wide (width > 1)**: CJK ideographs, full-width katakana/hiragana, full-width punctuation
- **Narrow (width = 1)**: ASCII, Latin scripts, half-width characters
- **Zero-width (None)**: Control characters, combining marks

**Text measurement:**

```rust
pub(crate) fn measure_text_width(
    text: &str,
    full_char_width: f32,
    char_width: f32,
) -> f32 {
    text.chars()
        .map(|c| measure_char_width(c, full_char_width, char_width))
        .sum()
}
```

This approach provides O(n) accurate width calculation for any string containing mixed ASCII and CJK characters.

#### IME (Input Method Editor) Support

**Location:** `canvas_editor/ime_requester.rs`

Asian languages require IME for input because they have thousands of characters that cannot be directly typed. The editor includes full IME support through the `ImeRequester` widget.

**Architecture:**

```rust
pub struct ImeRequester {
    enabled: bool,                  // IME state
    cursor: Rectangle,              // Cursor position in widget coordinates
    preedit: Option<Preedit>,       // Composition text before finalization
}
```

**How it works:**

1. **Invisible bridge**: `ImeRequester` is a zero-size widget that communicates with the OS IME system
2. **Coordinate conversion**: Converts editor-relative cursor position to window-relative coordinates
3. **Candidate window positioning**: Uses "over-the-spot" style to position IME candidate window near cursor
4. **Preedit synchronization**: Manages composition text (characters being typed but not yet finalized)

**Event handling:**

```rust
// On each RedrawRequested event
Event::RedrawRequested(_) => {
    if self.enabled {
        // Convert cursor from widget-relative to window-relative coordinates
        let window_cursor = Rectangle {
            x: self.cursor.x + layout.position().x,
            y: self.cursor.y + layout.position().y,
            // ...
        };
        
        // Request IME with updated cursor position
        shell.request_input_method(InputMethod::Enabled {
            cursor: window_cursor,
            purpose: None,  // Over-the-spot positioning
            preedit: self.preedit.clone(),
        });
    }
}
```

**Why RedrawRequested?**

IME candidate window positioning must use fresh cursor coordinates on every frame. This ensures the window follows cursor movement accurately, even during scrolling or window resize.

**Supported operations:**

- Enable/disable IME based on editor focus
- Position candidate window relative to cursor
- Display preedit (composition) text with selection
- Handle multi-character input sequences (e.g., typing "nihon" → 日本)

#### Rendering Integration

Character widths are critical for correct visual rendering throughout the editor.

**Cursor positioning** (`canvas_editor/mod.rs`):

When clicking with the mouse, `measure_text_width()` determines which character the cursor should be placed at:

```rust
// Calculate click position by accumulating character widths
let mut accumulated_width = 0.0;
for (char_index, c) in line_text.chars().enumerate() {
    let char_w = measure_char_width(c, self.full_char_width, self.char_width);
    if click_x < accumulated_width + (char_w / 2.0) {
        return char_index;  // Clicked before midpoint of character
    }
    accumulated_width += char_w;
}
```

**Selection rendering** (`canvas_editor/canvas_impl.rs:293-297`):

When rendering selections and syntax highlighting, x-offset is calculated using accurate character widths:

```rust
// In syntax highlighting loop
for (style, segment_text) in line_regions {
    // Calculate width of this colored segment
    let segment_width = measure_text_width(
        segment_text,
        self.full_char_width,
        self.char_width,
    );
    
    // Draw text at correct position
    frame.fill_text(Text { position: Point::new(x_offset, y), .. });
    
    // Advance position for next segment
    x_offset += segment_width;
}
```

**UTF-8 handling:**

All text operations properly handle UTF-8 character boundaries to prevent panics when slicing strings containing multi-byte CJK characters.

**Affected operations:**

- Mouse click → cursor positioning
- Text selection → rectangle geometry
- Syntax highlighting → segment positioning
- Horizontal scrolling → viewport calculations
- Find/replace → match highlighting

## Performance Considerations

### 1. Canvas Caching

```rust
self.content_cache = canvas::Cache::default();
self.overlay_cache = canvas::Cache::default();
self.content_cache.clear();
self.overlay_cache.clear();
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

### 5. CJK Character Width Calculation

**Character width measurement:** O(n) per visible line per frame

```rust
// Called for every visible line during rendering
let text_width = measure_text_width(line_text, full_char_width, char_width);
```

**Cost factors:**

- Iterates through all characters in visible text
- Unicode width lookup per character (fast hash table lookup)
- Summation of widths

**Optimization:**

- Only visible lines are measured (virtual scrolling)
- Width calculation is simple arithmetic (no complex geometry)
- Typical visible area: ~50 lines × ~100 chars = ~5,000 operations per frame

**Performance impact:**

- **Negligible** for typical files with mixed ASCII/CJK content
- **Acceptable** even for lines with 100% wide characters
- Much faster than actual text rendering and syntax highlighting

**Trade-off:** Accurate width calculation is essential for correct cursor positioning and selection rendering. The O(n) cost is unavoidable and well-optimized.

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
self.overlay_cache.clear();
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

Check [TODO.md](https://github.com/LuDog71FR/iced-code-editor/blob/main/TODO.md) for details.

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
