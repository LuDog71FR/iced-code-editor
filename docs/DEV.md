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
   - [Multi-Cursor Editing](#multi-cursor-editing)
   - [Line Wrapping (Visual Lines)](#line-wrapping-visual-lines)
   - [Code Folding](#code-folding)
   - [Search and Replace](#search-and-replace)
   - [Auto-Indentation](#auto-indentation)
   - [Cursor Blinking](#cursor-blinking)
   - [Focus Management](#focus-management)
   - [Selection Rendering](#selection-rendering)
   - [Scroll-to-Cursor](#scroll-to-cursor)
   - [Internationalization (i18n)](#internationalization-i18n)
   - [CJK and Asian Character Support](#cjk-and-asian-character-support)
5. [Language Server Protocol (LSP) Support](#language-server-protocol-lsp-support)
   - [Architecture](#architecture-1)
   - [Layer 1 — LspClient trait](#layer-1--lspclient-trait-canvas_editorlsprs)
   - [Layer 2 — LspProcessClient](#layer-2--lspprocessclient-canvas_editorlsp_processmodrs)
   - [Layer 3 — LspOverlayState + view_lsp_overlay](#layer-3--lspoverlaystate--view_lsp_overlay-canvas_editorlsp_processoverlayrs)
   - [Event flow](#event-flow)
6. [Performance Considerations](#performance-considerations)
   - [Canvas Caching](#1-canvas-caching)
   - [Syntax Highlighting Optimization](#2-syntax-highlighting-optimization)
   - [Text Buffer Performance](#3-text-buffer-performance)
   - [Memory Usage](#4-memory-usage)
   - [CJK Character Width Calculation](#5-cjk-character-width-calculation)
7. [Testing Strategy](#testing-strategy)
   - [Unit Tests](#unit-tests)
   - [Integration Tests](#integration-tests)
   - [Running Tests](#running-tests)
   - [Benchmarks](#benchmarks)
8. [Common Pitfalls](#common-pitfalls)
   - [UTF-8 Character Boundaries](#1-utf-8-character-boundaries)
   - [Cache Invalidation](#2-cache-invalidation)
   - [Command History Grouping](#3-command-history-grouping)
   - [Selection Direction](#4-selection-direction)
9. [Future Enhancements](#future-enhancements)
10. [Contributing Guidelines](#contributing-guidelines)
    - [Code Style](#code-style)
    - [Pull Request Process](#pull-request-process)
    - [Commit Messages](#commit-messages)
    - [Documentation](#documentation)
11. [Resources](#resources)
    - [Iced Framework](#iced-framework)
    - [Syntax Highlighting](#syntax-highlighting-1)
    - [Design Patterns](#design-patterns-1)
    - [Text Editor Algorithms](#text-editor-algorithms)
12. [License](#license)

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
├── i18n.rs                   # Internationalization (rust-i18n)
└── canvas_editor/            # Core editor implementation
    ├── mod.rs                # Main editor struct, builder API, constants
    ├── canvas_impl.rs        # Canvas rendering (Iced Canvas trait)
    ├── clipboard.rs          # Clipboard operations
    ├── command.rs            # Command pattern for undo/redo
    ├── cursor.rs             # Cursor movement logic
    ├── cursor_set.rs         # Multi-cursor collection (Cursor / CursorSet)
    ├── folding.rs            # Code folding (foldable region detection)
    ├── history.rs            # Command history management
    ├── ime_requester.rs      # IME bridge widget (CJK input)
    ├── search.rs             # Search/replace state and matching
    ├── search_dialog.rs      # Search/replace dialog UI
    ├── selection.rs          # Text selection logic
    ├── update.rs             # Message handling (Elm Architecture)
    ├── view.rs               # UI view construction
    ├── wrapping.rs           # Line wrapping (logical ↔ visual lines)
    ├── lsp.rs                # LspClient trait + LSP data types
    └── lsp_process/          # LSP subprocess client (feature: lsp-process)
        ├── mod.rs            # LspProcessClient (stdio JSON-RPC)
        ├── config.rs         # Per-server configuration
        └── overlay.rs        # Hover / completion overlay UI
```

### Core Components

#### 1. **CodeEditor** (`canvas_editor/mod.rs`)

The main widget struct that holds all editor state:

```rust
pub struct CodeEditor {
    buffer: TextBuffer,                  // Text content
    cursors: cursor_set::CursorSet,      // Multi-cursor set (primary + extras)
    style: Style,                        // Visual theme
    syntax: String,                      // Language for highlighting
    history: CommandHistory,             // Undo/redo system
    content_cache: canvas::Cache,        // Text/gutter layer (stable)
    overlay_cache: canvas::Cache,        // Cursor/selection/search layer
    viewport_scroll: f32,                // Vertical scroll (pixels)
    horizontal_scroll_offset: f32,       // Horizontal scroll (no-wrap mode)
    wrap_enabled: bool,                  // Line wrapping toggle
    wrap_column: Option<usize>,          // Fixed wrap column (or viewport)
    folding_enabled: bool,               // Code folding toggle
    collapsed_folds: HashSet<usize>,     // Collapsed region headers
    auto_indent_enabled: bool,           // Auto-indent on newline
    indent_style: IndentStyle,           // Spaces(n) or Tab
    search_state: search::SearchState,   // Search/replace state
    lsp_client: Option<Box<dyn LspClient>>, // Optional LSP connection
    highlight_cache: RefCell<Option<HighlightCache>>, // Sequential span cache
    visual_lines_cache: RefCell<Option<VisualLinesCache>>, // Wrapping cache
    // ... revisions, viewport metrics, font metrics, IME state, etc.
}
```

**Key characteristics:**

- Single source of truth for editor state
- No external dependencies on text buffer format
- All state transitions happen through message handling
- Derived layout (wrapping, highlighting) is memoized in `RefCell` caches keyed
  by monotonic revision counters (`buffer_revision`, `fold_revision`)

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

**Two-layer cache optimization:**

Rendering is split across **two** `canvas::Cache` layers so that frequent visual
changes do not invalidate the expensive text geometry:

- **`content_cache`** — syntax-highlighted glyphs and the line-number gutter.
  Intentionally kept stable across cursor/selection movement, so mouse-drag
  selection stays smooth. Cleared only when the buffer, syntax, theme, or layout
  (wrap/fold) changes.
- **`overlay_cache`** — cursor and current-line highlight, selection rectangles,
  search-match highlights and IME preedit decorations. Cleared on every cursor
  blink, selection drag and search update.

```rust
self.content_cache.clear();  // buffer / layout changed
self.overlay_cache.clear();  // cursor / selection / search changed
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

**Integration:** Uses the `syntect` crate. The optional `two-face` dependency adds
extra Sublime syntax/theme definitions beyond syntect's defaults.

Highlighting is **not** recomputed naïvely per frame. Instead, each logical line is
tokenized once and memoized as a dense per-line prefix that also stores the syntect
parser/highlight state left *after* the line, so multi-line constructs (block
comments, multi-line strings) resume correctly:

```rust
// canvas_impl.rs — resumes from the cached state of line N-1
let spans = self.highlighted_line_cached(logical_line, syntax, theme, syntax_set);
```

**Key points:**

- A line is tokenized once and reused across wrapped visual segments and across
  scroll-only renders.
- On an edit, the cache is *truncated* from the first changed line (tracked via
  `pre_edit_line`) rather than fully cleared, so typing re-highlights only from the
  edited line down.
- `highlight_line_spans()` (independent, single-line) is retained for tests and
  benchmarks.

See [Syntax Highlighting Optimization](#2-syntax-highlighting-optimization) for the
full cache and invalidation strategy.

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

### Multi-Cursor Editing

**Location:** `canvas_editor/cursor_set.rs`

The editor supports multiple simultaneous cursors. State lives in a `CursorSet`,
an ordered, deduplicated collection that always contains at least one cursor — the
**primary** cursor, which the viewport follows and which receives IME input.

```rust
pub struct Cursor {
    pub position: (usize, usize),       // (line, col)
    pub anchor: Option<(usize, usize)>, // selection start (None = no selection)
}

pub struct CursorSet {
    cursors: Vec<Cursor>,  // kept sorted in document order
    primary_idx: usize,    // index of the primary cursor
}
```

**Invariants and behaviour:**

- Cursors are kept sorted in document order after any mutation that may reorder them.
- `sort_and_merge()` collapses cursors that share a position or whose selections
  overlap, so duplicate/overlapping cursors can never coexist. The primary index is
  tracked through the merge so it keeps pointing at the same logical cursor.
- Each cursor carries its own selection (`anchor` → `position`); a per-cursor
  `selection_range()` returns the normalised `(start, end)` pair.

**Editor integration:**

- `set_single(pos)` collapses back to one cursor (normal click / arrow movement).
- `add_cursor(pos)` / `add_cursor_with_selection(c)` add a secondary cursor and make
  it primary (e.g. Alt+Click, "add cursor at next match").
- `remove_all_but_primary()` restores single-cursor mode (Esc).
- Text commands are applied at every cursor; `get_selection_range()` in
  `selection.rs` delegates to the primary cursor (see [Selection Direction](#4-selection-direction)).

### Line Wrapping (Visual Lines)

**Location:** `canvas_editor/wrapping.rs`

When wrapping is enabled, a single **logical line** (as stored in the buffer) may be
displayed as several **visual lines**. All rendering, scrolling and cursor math
operate on visual lines; the buffer remains unwrapped.

```rust
pub struct VisualLine {
    pub logical_line: usize,   // source line in the buffer
    pub segment_index: usize,  // 0 = first segment, 1+ = wrapped continuation
    pub start_col: usize,      // inclusive start column in the logical line
    pub end_col: usize,        // exclusive end column
}
```

`WrappingCalculator` converts the buffer into a `Vec<VisualLine>`:

- **Viewport wrapping** (`wrap_column = None`): wraps at the available pixel width
  (viewport width minus the gutter), using the CJK-aware character widths.
- **Fixed-column wrapping** (`wrap_column = Some(n)`): wraps at `n` characters.
- **Folding-aware**: logical lines hidden by collapsed folds produce no visual lines
  (the `hidden` set is passed in — see [Code Folding](#code-folding)).
- `logical_to_visual()` maps a buffer position to its visual line for cursor placement.

The result is memoized in `visual_lines_cache`, keyed by buffer revision, viewport
and gutter width, wrap settings, fold revision and font metrics, so wrapping is only
recomputed when one of those inputs changes.

### Code Folding

**Location:** `canvas_editor/folding.rs` (logic), `canvas_editor/mod.rs` (state)

Folding lets the user collapse indented blocks. Detection is **indentation-based**
and therefore language-agnostic: a line is a fold header when the next non-blank line
is more deeply indented (the same fallback strategy VS Code uses).

```rust
pub struct FoldRegion {
    pub start_line: usize, // header line — stays visible when collapsed
    pub end_line: usize,   // last line of the region — hidden when collapsed
}

pub fn compute_foldable_regions(buffer: &TextBuffer) -> Vec<FoldRegion>;
pub fn hidden_lines(regions: &[FoldRegion], collapsed: &HashSet<usize>) -> HashSet<usize>;
```

**State and flow:**

- `collapsed_folds: HashSet<usize>` stores the header lines that are currently
  collapsed; `toggle_fold(header_line)` / `toggle_fold_at(line)` flip them.
- `fold_revision` is bumped on every fold change so the visual-lines cache is
  invalidated.
- `foldable_regions_cache` memoizes detection keyed by `buffer_revision`.
- At render time, `hidden_lines()` produces the set of hidden logical lines, which is
  fed to the `WrappingCalculator` so collapsed lines simply disappear from layout.
- Trailing blank lines are trimmed from a region so a collapsed block does not swallow
  the gap before the next block; nested blocks each yield independent regions.

### Search and Replace

**Location:** `canvas_editor/search.rs` (state/matching), `canvas_editor/search_dialog.rs` (UI)

A built-in find/replace dialog, gated by `search_replace_enabled`.

```rust
pub struct SearchState {
    pub query: String,
    pub replace_with: String,
    pub case_sensitive: bool,
    pub is_open: bool,
    pub is_replace_mode: bool,          // search-only vs search+replace
    pub matches: Vec<SearchMatch>,      // all matches in the buffer
    pub current_match_index: Option<usize>,
    pub focused_field: SearchFocusedField, // Tab navigation
    // input IDs ...
}
```

**Behaviour:**

- `find_matches()` scans the buffer and returns every `SearchMatch { line, col }`
  (columns are UTF-8 character offsets). Re-run on query change or case toggle.
- `next_match()` / `previous_match()` cycle through results; `select_match_near_cursor()`
  jumps to the match closest to the caret when the dialog opens.
- Matches are highlighted in the `overlay_cache` layer; only the visible match range
  is drawn (`get_visible_match_range()`).
- All dialog labels and placeholders are localized through the i18n layer.

### Auto-Indentation

**Location:** `canvas_editor/mod.rs`, `canvas_editor/update.rs`

When `auto_indent_enabled` is set, pressing Enter copies the leading whitespace of the
current line onto the new line. The inserted whitespace itself follows the configured
indentation style:

```rust
pub enum IndentStyle {
    Spaces(u8), // insert `n` space characters
    Tab,        // insert a single '\t'
}

// Standard presets offered to the UI:
IndentStyle::ALL == [Spaces(2), Spaces(4), Spaces(8), Tab];
```

`set_indent_style()` selects the active style and `set_auto_indent_enabled()` toggles
the behaviour. Tab width for display/folding is governed by the `TAB_WIDTH` constant.

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

**Normalization:** Selections are normalized before rendering. With multi-cursor
support, the anchor/position pair lives on each `Cursor`; `get_selection_range()`
delegates to the primary cursor, which normalises via `selection_range()` in
`cursor_set.rs` (see [Selection Direction](#4-selection-direction)).

```rust
// cursor_set.rs — start is guaranteed to be before end in document order
pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
    let anchor = self.anchor?;
    if anchor == self.position {
        return None;
    }
    Some(normalise(anchor, self.position))
}
```

**Rendering:**

- Every cursor with an active selection is rendered.
- Single-line selection: a single rectangle.
- Multi-line selection: three rectangles (first line, middle block, last line).

### Scroll-to-Cursor

**Auto-scrolling:** Cursor always stays visible

```rust
pub fn scroll_to_cursor(&self) -> Task<Message> {
    let cursor_y = self.cursors.primary_position().0 as f32 * LINE_HEIGHT;

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
    English, French, Spanish, German, Italian,
    PortugueseBR, PortuguesePT, ChineseSimplified,
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

**Currently shipped locales:** `en`, `fr`, `es`, `de`, `it`, `pt-BR`, `pt-PT`, `zh-CN`.

**Adding a new language:**

1. Create `locales/ja.yml` with translation keys
2. Add `Japanese` to the `Language` enum
3. Update `to_locale()` to return `"ja"`
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

## Language Server Protocol (LSP) Support

**Feature gate:** `lsp-process` (not available on WASM)

**Location:** `canvas_editor/lsp.rs`, `canvas_editor/lsp_process/`

### Architecture

The LSP integration is split into three layers:

```
┌─────────────────────────────────────────────────────┐
│  Application (demo-app)                             │
│  ┌──────────────┐  ┌────────────────────────────┐  │
│  │ app_lsp.rs   │  │ ui/lsp.rs                  │  │
│  │ timers/events│  │ view_lsp_overlay() wrapper  │  │
│  └──────┬───────┘  └─────────────┬──────────────┘  │
└─────────│─────────────────────────│─────────────────┘
          │                         │
┌─────────│─────────────────────────│─────────────────┐
│  Library (iced-code-editor)       │                  │
│  ┌──────▼───────┐  ┌─────────────▼──────────────┐  │
│  │ LspClient    │  │ LspOverlayState             │  │
│  │ (trait)      │  │ + view_lsp_overlay()        │  │
│  └──────┬───────┘  └────────────────────────────┘  │
│  ┌──────▼───────┐                                   │
│  │LspProcessClient│ (lsp_process/mod.rs)            │
│  │ stdio subprocess│                                │
│  └──────────────┘                                   │
└─────────────────────────────────────────────────────┘
```

### Layer 1 — `LspClient` trait (`canvas_editor/lsp.rs`)

The `LspClient` trait decouples the editor from any particular LSP transport:

```rust
pub trait LspClient {
    fn did_open(&mut self, document: &LspDocument, text: &str);
    fn did_change(&mut self, document: &LspDocument, changes: &[LspTextChange]);
    fn did_save(&mut self, document: &LspDocument, text: &str);
    fn did_close(&mut self, document: &LspDocument);
    fn request_hover(&mut self, document: &LspDocument, position: LspPosition);
    fn request_completion(&mut self, document: &LspDocument, position: LspPosition);
    fn request_definition(&mut self, document: &LspDocument, position: LspPosition);
}
```

`CodeEditor` holds an `Option<Box<dyn LspClient>>` and calls the trait methods automatically when the document changes or the user requests hover/completion.

### Layer 2 — `LspProcessClient` (`canvas_editor/lsp_process/mod.rs`)

The concrete implementation communicates with an LSP server subprocess via **stdin/stdout** using the JSON-RPC framing of the Language Server Protocol:

- **Writer thread** — serialises requests and writes them to stdin
- **Reader thread** — reads and parses server responses, routes them by request ID
- **Stderr thread** — forwards server log lines as `LspEvent::Log`

All three `JoinHandle`s are stored as `_writer_thread`, `_reader_thread`, and `_stderr_thread` fields on `LspProcessClient`, so the threads are never detached. The `Drop` implementation sends LSP `shutdown` / `exit` notifications, then kills the child process; the threads terminate naturally when their I/O streams reach EOF.

Events are sent back to the application through an `mpsc::Sender<LspEvent>`:

```rust
pub enum LspEvent {
    Hover { text: String },
    Completion { items: Vec<String> },
    Definition { uri: String, range: LspRange },
    Progress { token, server_key, title, message, percentage, done },
    Log { server_key, message },
}
```

Server configurations (command, arguments, language IDs) live in `lsp_process/config.rs` and are keyed by a short string such as `"lua-language-server"` or `"rust-analyzer"`.

**UTF-16 conversion:** LSP uses UTF-16 character offsets while the editor works in UTF-8. `TextModel` inside `LspProcessClient` mirrors the document content and converts positions before every request.

### Layer 3 — `LspOverlayState` + `view_lsp_overlay` (`canvas_editor/lsp_process/overlay.rs`)

All display-related state is aggregated in `LspOverlayState`:

| Field | Role |
|---|---|
| `hover_text` / `hover_items` | Raw text + parsed markdown for the tooltip |
| `hover_visible` / `hover_position` | Tooltip visibility and anchor point |
| `hover_interactive` | True while the mouse is over the tooltip (prevents auto-hide) |
| `all_completions` / `completion_filter` | Full list + current filter string |
| `completion_items` | Filtered items actually displayed |
| `completion_visible` / `completion_selected` | Menu visibility and keyboard selection |
| `completion_suppressed` | Prevents re-showing after an item is applied |
| `completion_position` | Anchor point for the menu |

`view_lsp_overlay()` is a generic function parameterised over the application message type `M`. It takes a mapping function `f: impl Fn(LspOverlayMessage) -> M` and renders a `stack![]` of three layers:

1. **Base** — fills the editor viewport
2. **Completion layer** — scrollable item list, positioned above or below the cursor
3. **Hover layer** — scrollable markdown tooltip, positioned left or right of the token

Both overlays compute their position at render time from editor viewport measurements (`viewport_width`, `viewport_height`, `viewport_scroll`, `char_width`).

### Event flow

```
User moves mouse  →  CodeEditor emits MouseHover(point)
                  →  App calls editor.lsp_hover_anchor_at_point()
                  →  LspHoverPending queued (`LSP_HOVER_REQUEST_DELAY_MS` delay)
                  →  Tick fires: editor.lsp_request_hover_at_position()
                  →  LspProcessClient sends hover request to server
                  →  Server replies → LspEvent::Hover { text }
                  →  App calls overlay.show_hover(text)
                  →  view_lsp_overlay() renders the tooltip

User types char   →  CodeEditor emits CharacterInput
                  →  LspProcessClient sends didChange
                  →  Server replies → LspEvent::Completion { items }
                  →  App calls overlay.set_completions(items, cursor_pos)
                  →  view_lsp_overlay() renders the completion menu
```

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

**Current:** Highlighting is sequential and memoized as a dense per-line prefix.

`CodeEditor.highlight_cache` (a `RefCell<Option<HighlightCache>>`) stores, for each logical line, its colored spans **and** the syntect parser/highlight state left *after* that line. To highlight line `N`, `highlighted_line_cached()` resumes from the state of line `N - 1`, so multi-line constructs (block comments, multi-line strings) are colored correctly:

```rust
// Lines 0..=logical_line are tokenized in order, resuming state across lines;
// the prefix is reused on later calls.
let spans = self.highlighted_line_cached(logical_line, syntax, theme, syntax_set);
```

**Invalidation (incremental):**

- On an edit, the prefix is **truncated** from the first changed line rather than fully cleared. The first changed line is bounded by `pre_edit_line`, captured at the top of `update()` from the topmost active cursor/selection (with a one-line margin for merges). Lines before it keep their cached spans and states.
- Operations whose changes are not anchored to a single line — undo/redo and Replace All — reset the prefix entirely (`pre_edit_line = 0`); these are rare and not on the typing path.
- The cache is also reset when the active syntax changes; `reset()` clears it on content replacement.

**Consequences:**

- A line is tokenized once and reused across wrapped visual segments and across renders; scroll-only renders reuse the prefix.
- Typing re-highlights only from the edited line down, not from the top of the file.
- Character→byte conversions in the draw loop use `char_range_to_byte_range()` (single pass) instead of repeated `char_indices().nth()` (`O(n)` per boundary).
- `highlight_line_spans()` (independent, single-line) is retained for tests and benchmarks.

**Future improvements:**

- Background parsing for large files.
- Bounded/checkpointed state cache to cap memory when scrolling very large files (states are currently stored per highlighted line).
- Faster regex backend (`fancy-regex` → `oniguruma`), at the cost of a C dependency.

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

### Benchmarks

Performance-critical hot paths are benchmarked with [criterion](https://github.com/bheisler/criterion.rs). The benchmarks live in `iced-code-editor/benches/editor_benchmarks.rs` and measure the work performed per edit / per scroll on a synthetic 10,000-line source file:

| Benchmark | Function under test |
|---|---|
| `highlight_line_spans` | Tokenizing one line into colored spans |
| `calculate_visual_lines_10k` | Line wrapping (`WrappingCalculator`) |
| `compute_foldable_regions_10k` | Fold-region detection |
| `find_matches_10k` | Search across the buffer |

**Feature gate:** These functions are internal, so they are exposed to the benchmark crate through the hidden `bench_support` module (`canvas_editor/mod.rs`, re-exported from `lib.rs`). Both the module and the `[[bench]]` target are gated behind the `bench` feature (`required-features = ["bench"]`), so the benchmarks are invisible to normal builds and to the public API.

**Running:**

```bash
# The bench feature is mandatory — required-features won't enable it automatically
cargo bench -p iced-code-editor --features bench

# Run a single benchmark by name
cargo bench -p iced-code-editor --features bench -- highlight_line_spans
```

criterion stores results under `target/criterion/` and automatically reports the delta against the previous run, so the workflow is: benchmark once to establish a baseline, make a change, then benchmark again to see the regression or improvement. When `gnuplot` is unavailable, criterion falls back to the bundled `plotters` backend for the HTML reports.

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
self.cursors.set_single(new_position);
self.overlay_cache.clear();
```

### 3. Command History Grouping

**Problem:** Forgetting to end groups leaves consecutive operations merged into a single undo step (broken undo boundaries)

**Solution:** Always pair the start and end of a group. The grouping logic is encapsulated in two helpers in `update.rs` that guard on the `is_grouping` flag:

```rust
// Begin grouping on the first edit of a typing run
fn ensure_grouping_started(&mut self, label: &str) {
    if !self.is_grouping {
        self.history.begin_group(label);
        self.is_grouping = true;
    }
}

// End grouping on navigation, deletion, or a new operation type
fn end_grouping_if_active(&mut self) {
    if self.is_grouping {
        self.history.end_group();
        self.is_grouping = false;
    }
}
```

### 4. Selection Direction

**Problem:** User can drag selection backwards

**Solution:** Always normalize selection ranges. With multi-cursor support, `get_selection_range()` delegates to the primary cursor, which normalizes via `normalise()` in `cursor_set.rs`:

```rust
// selection.rs
pub(crate) fn get_selection_range(
    &self,
) -> Option<((usize, usize), (usize, usize))> {
    self.cursors.primary().selection_range()
}

// cursor_set.rs — start is guaranteed to be before end
pub fn selection_range(&self) -> Option<((usize, usize), (usize, usize))> {
    let anchor = self.anchor?;
    if anchor == self.position {
        return None;
    }
    Some(normalise(anchor, self.position))
}
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
