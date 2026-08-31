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
   - [Generated Boilerplate for Boolean Options](#6-generated-boilerplate-for-boolean-options)
4. [Key Implementation Details](#key-implementation-details)
   - [Syntax Highlighting](#syntax-highlighting)
   - [Virtual Scrolling](#virtual-scrolling)
   - [Multi-Cursor Editing](#multi-cursor-editing)
   - [Line Wrapping (Visual Lines)](#line-wrapping-visual-lines)
   - [Code Folding](#code-folding)
   - [Sticky Scroll](#sticky-scroll)
   - [Search and Replace](#search-and-replace)
   - [Command Palette](#command-palette)
   - [Context Menu and Shared Actions](#context-menu-and-shared-actions)
   - [Auto-Indentation](#auto-indentation)
   - [Auto-Closing Brackets/Quotes](#auto-closing-bracketsquotes)
   - [Matching Bracket/Quote Highlight](#matching-bracketquote-highlight)
   - [Bracket-Pair Colorization](#bracket-pair-colorization)
   - [Indentation Guides](#indentation-guides)
   - [Color Preview Swatches](#color-preview-swatches)
   - [Vim Emulation](#vim-emulation)
   - [Cursor Blinking](#cursor-blinking)
   - [Focus Management](#focus-management)
   - [Selection Rendering](#selection-rendering)
   - [Scroll-to-Cursor](#scroll-to-cursor)
   - [Internationalization (i18n)](#internationalization-i18n)
   - [CJK and Asian Character Support](#cjk-and-asian-character-support)
5. [Language Server Protocol (LSP) Support](#language-server-protocol-lsp-support)
   - [Architecture](#architecture-1)
   - [Layer 1 — `LspClient` trait (`canvas_editor/lsp/mod.rs`)](#layer-1--lspclient-trait-canvas_editorlspmodrs)
   - [Layer 2 — `LspProcessClient` (`canvas_editor/lsp/process/mod.rs`)](#layer-2--lspprocessclient-canvas_editorlspprocessmodrs)
   - [Layer 3 — `LspOverlayState` + `view_lsp_overlay` (`canvas_editor/lsp/process/overlay.rs`)](#layer-3--lspoverlaystate--view_lsp_overlay-canvas_editorlspprocessoverlayrs)
   - [Applying server edits (`canvas_editor/lsp/edits.rs`)](#applying-server-edits-canvas_editorlspeditsrs)
   - [Event flow](#event-flow)
6. [Performance Considerations](#performance-considerations)
   - [Canvas Caching](#1-canvas-caching)
   - [Syntax Highlighting Optimization](#2-syntax-highlighting-optimization)
   - [Text Buffer Performance](#3-text-buffer-performance)
   - [Memory Usage](#4-memory-usage)
   - [CJK Character Width Calculation](#5-cjk-character-width-calculation)
7. [Testing Strategy](#testing-strategy)
   - [The three levels](#the-three-levels)
   - [Unit Tests](#unit-tests)
   - [Interface Tests (`demo-app/src/ui_tests/`)](#interface-tests-demo-appsrcui_tests)
   - [Integration Tests](#integration-tests)
   - [Regression tests must be verified failing](#regression-tests-must-be-verified-failing)
   - [Running Tests](#running-tests)
   - [Benchmarks](#benchmarks)
8. [Common Pitfalls](#common-pitfalls)
   - [UTF-8 Character Boundaries](#1-utf-8-character-boundaries)
   - [Cache Invalidation](#2-cache-invalidation)
   - [Command History Grouping](#3-command-history-grouping)
   - [Selection Direction](#4-selection-direction)
   - [Multi-Cursor Edit Order](#5-multi-cursor-edit-order)
   - [Buffer Revision Bumping](#6-buffer-revision-bumping)
   - [Highlight Cache Anchor (`pre_edit_line`)](#7-highlight-cache-anchor-pre_edit_line)
   - [InsertTextCommand Cursor Override vs. Undo](#8-inserttextcommand-cursor-override-vs-undo)
9. [Future Enhancements](#future-enhancements)
10. [Contributing Guidelines](#contributing-guidelines)
    - [Code Style](#code-style)
    - [Pull Request Process](#pull-request-process)
    - [Commit messages](#commit-messages)
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

The workspace has three members (`Cargo.toml`):

- **`iced-code-editor/`** — the library (the only default member)
- **`demo-app/`** — the full-featured demo, which also carries the widget-level
  interface test suite (see [Testing Strategy](#testing-strategy))
- **`simple-example/`** — a minimal embedding, kept small on purpose so the
  smallest useful integration stays visible and compiling

The library follows a modular architecture with clear separation of concerns:

```
iced-code-editor/
├── lib.rs                    # Public API and documentation
├── theme.rs                  # Styling and theming system
├── i18n.rs                   # Internationalization (rust-i18n)
├── buffer/                   # Text storage
│   ├── mod.rs                 # Gap-buffer text storage + line-ending round-trip
│   └── text_utils.rs          # UTF-8 char-offset <-> byte-offset helpers
└── canvas_editor/            # Core editor implementation
    ├── mod.rs                # CodeEditor struct, Message enum, new()/reset()
    ├── metrics.rs             # Font/char/line/viewport dimension constants + methods
    ├── caches.rs              # Visual-line, highlight, bracket-depth, max-width caches
    ├── config.rs              # Builder-style set_*/with_*/getter configuration methods
    ├── bool_options.rs        # Macro generating the boolean options' getters/builders
    ├── focus.rs               # Editor focus management
    ├── bench_support.rs       # Criterion benchmark harness (feature: bench)
    ├── editing/               # Cursor, selection, clipboard, undo/redo
    │   ├── mod.rs
    │   ├── cursor.rs           # Cursor movement/positioning, paging, scroll-to-cursor
    │   ├── cursor_set.rs       # Multi-cursor collection (Cursor / CursorSet)
    │   ├── selection.rs        # Text selection logic
    │   ├── clipboard.rs        # Clipboard operations
    │   ├── history.rs          # Command history management
    │   └── command/            # Command pattern for undo/redo
    │       ├── mod.rs           # The Command trait
    │       ├── edit.rs          # Char/newline/range editing commands
    │       ├── composite.rs     # Composite/replace commands
    │       ├── lines.rs         # Move/duplicate line commands
    │       └── comment.rs       # Line-comment toggle command
    ├── input/                 # Event routing and Message dispatch
    │   ├── mod.rs
    │   ├── events.rs           # Keyboard/mouse/IME event handling
    │   ├── shortcuts.rs        # Key-combination recognition (command_pressed, ...)
    │   ├── ime_requester.rs    # IME bridge widget (CJK input)
    │   └── update/            # Message handling (Elm Architecture), one file per group
    │       ├── mod.rs           # Shared helpers: finish_*, adjust_other_cursors, grouping
    │       ├── dispatch.rs      # Top-level `pub fn update()` message match
    │       ├── text_input.rs    # Character input, Tab, auto-close/auto-indent
    │       ├── deletion.rs      # Backspace, Delete, delete-selection
    │       ├── navigation.rs    # Arrows, Home/End, Page Up/Down, goto position
    │       ├── clipboard.rs     # Cut/copy/paste messages
    │       ├── mouse.rs         # Click, drag, double/triple click, context menu
    │       ├── multi_cursor.rs  # Alt+Click, add cursor above/below, next occurrence
    │       ├── line_ops.rs      # Move/duplicate line, toggle comment
    │       ├── history_ops.rs   # Undo/redo
    │       ├── focus_ime.rs     # Focus gained/lost, IME preedit/commit
    │       └── scroll_timer.rs  # Scroll messages and the blink/scroll tick
    ├── render/                 # Rendering
    │   ├── mod.rs
    │   ├── canvas.rs            # canvas::Program trait implementation
    │   ├── text.rs              # Glyph drawing, tab/whitespace expansion, guides
    │   ├── highlighting.rs      # Syntax resolution, per-line highlight cache, palettes
    │   ├── gutter.rs            # Line numbers, wrap indicators, fold chevrons
    │   ├── overlays.rs          # Selection/cursor/search highlight drawing
    │   ├── wrapping.rs          # Line wrapping (logical <-> visual lines)
    │   └── view.rs              # Iced UI view construction
    ├── features/                # Optional editor features
    │   ├── mod.rs
    │   ├── actions.rs            # SharedAction, ActionContext, shortcut hint strings
    │   ├── bracket_match.rs      # Matching bracket/quote detection + depth scanning
    │   ├── color_preview.rs      # Inline color-literal detection (pure logic)
    │   ├── sticky_scroll.rs      # Pinned enclosing-block headers (pure logic)
    │   ├── context_menu.rs       # Right-click context menu
    │   ├── command_palette/      # Command palette (Ctrl+Shift+P)
    │   │   ├── mod.rs             # State, command registry, subsequence filtering
    │   │   ├── dialog.rs          # Palette UI + arrow/Escape key listener
    │   │   └── update.rs
    │   ├── folding/              # Code folding
    │   │   ├── mod.rs             # Foldable-region detection (pure logic)
    │   │   └── ops.rs             # Fold/unfold operations on CodeEditor
    │   ├── indent_guides.rs      # Indentation-guide level computation (pure logic)
    │   ├── goto_line/            # Go-to-line dialog
    │   │   ├── mod.rs
    │   │   ├── dialog.rs
    │   │   └── update.rs
    │   ├── search/                # Search/replace
    │   │   ├── mod.rs             # Search state and matching
    │   │   ├── dialog.rs          # Search/replace dialog UI
    │   │   └── update.rs
    │   └── vim/                   # Vim emulation
    │       ├── mod.rs
    │       └── update.rs
    └── lsp/                      # LSP integration
        ├── mod.rs                 # LspClient trait + LSP data types
        ├── edits.rs               # Applying a server TextEdit[] reply
        ├── sync.rs                # Buffer <-> LSP document synchronization
        └── process/               # LSP subprocess client (feature: lsp-process)
            ├── mod.rs              # LspProcessClient (process lifecycle, LspClient impl)
            ├── protocol.rs         # JSON-RPC framing, bounded reads, response parsing
            ├── text_model.rs       # Per-document UTF-16 position mirror
            ├── pending.rs          # In-flight request tracking
            ├── config.rs           # Per-server configuration
            └── overlay.rs          # Hover / completion overlay UI
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
    auto_close_brackets: bool,           // Auto-close brackets/quotes + surround selection
    bracket_match_highlight_enabled: bool, // Matching bracket/quote highlight overlay
    bracket_pair_colorization_enabled: bool, // Rainbow-bracket depth coloring
    indent_style: IndentStyle,           // Spaces(n) or Tab
    search_state: search::SearchState,   // Search/replace state
    lsp_client: Option<Box<dyn LspClient>>, // Optional LSP connection
    highlight_cache: RefCell<Option<HighlightCache>>, // Sequential span cache
    bracket_depth_cache: RefCell<BracketDepthCache>, // Sequential nesting-depth cache
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

#### 2. **TextBuffer** (`buffer/mod.rs`)

A line-based text storage optimized for editor operations. Lines are held
around a **movable gap** rather than in a single flat vector:

```rust
pub struct TextBuffer {
    lines_before: Vec<String>,  // lines before the gap, in document order
    lines_after: Vec<String>,   // lines after the gap, in *reverse* order
    line_ending: LineEnding,    // Lf | CrLf, detected on load
    trailing_newline: bool,     // did the source end with a terminator?
}
```

**Why a gap.** Editing is local: consecutive keystrokes land on the same line
or the one next to it. `move_gap_to(index)` walks the boundary to the edit
site once, after which inserting or removing a nearby line is O(1) — it pushes
or pops the top of one of the two vectors — instead of shifting the whole tail
of a `Vec<String>`. This is the locality principle behind piece-table editors,
kept compatible with the editor's existing borrowed `&str` line API.

**Design decisions:**

- **Line-based storage**: fast random access for virtual scrolling. `line(i)`
  stays O(1): the index either falls in `lines_before` directly, or is mirrored
  into `lines_after`, which is stored reversed precisely so its *front* (the
  gap side) is the cheap `Vec` end.
- **No rope**: the gap covers the editing pattern this editor actually has.
  A rope would win on huge block insertions far from the cursor, which is not
  the hot path.
- **UTF-8 aware**: columns are character indices, converted through
  `text_utils.rs` (see [UTF-8 Character Boundaries](#1-utf-8-character-boundaries)).

**Line endings round-trip.** `str::lines()` discards the terminator and
normalizes `\r\n` to `\n`, so a naive buffer silently converts a CRLF file to
LF on save. Two fields prevent that:

- `line_ending` is set by `LineEnding::detect()`, a majority vote — CRLF if at
  least half the newlines are part of a `\r\n` pair, otherwise LF. Content with
  no newline at all defaults to LF.
- `trailing_newline` records whether the source ended with a terminator, so a
  final newline is neither invented nor dropped.

`to_string()` reproduces both, so load → save is byte-identical when nothing
was edited.

**Operations:**

- `insert_char()` - Insert single character
- `insert_newline()` - Split line at position
- `delete_char()` - Delete before cursor (backspace)
- `delete_forward()` - Delete at cursor (delete key)

Every mutation goes through `line_mut()` or `move_gap_to()`, so the gap follows
the edit; nothing else in the codebase touches the two vectors directly.

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

**Location:** `canvas_editor/editing/command/`, `canvas_editor/editing/history.rs`

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
history.begin_group();
// ... multiple InsertCharCommand ...
history.end_group();  // Now undoable as single operation
```

**Benefits:**

- Complete undo/redo support
- Command grouping for natural undo boundaries
- Save point tracking for modified state detection
- Configurable history size for memory management

### 2. Elm Architecture (Message-Update-View)

**Location:** `canvas_editor/input/update/`, `canvas_editor/render/view.rs`

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

- **`editing/cursor.rs`** - Cursor movement, paging, scroll-to-cursor
- **`editing/selection.rs`** - Text selection logic and range calculations
- **`editing/clipboard.rs`** - Copy/paste operations
- **`render/canvas.rs`** - The `canvas::Program` impl and the draw entry point
- **`render/text.rs`** / **`render/highlighting.rs`** - Drawing glyphs vs.
  deciding their colors
- **`input/events.rs`** / **`input/shortcuts.rs`** - Raw event routing vs.
  recognizing a key combination as a command
- **`input/update/`** - Message handling, split one file per message group

Two splits are worth knowing about, because older notes and commit messages
still refer to the pre-split names:

- **`update.rs` no longer exists.** It became `input/update/`, where
  `mod.rs` holds the *shared helpers* every handler calls
  (`finish_edit_operation`, `adjust_other_cursors`, `min_active_line`, the
  grouping pair) and each sibling file holds the *handlers* for one group of
  messages — `text_input.rs`, `deletion.rs`, `navigation.rs`, `clipboard.rs`,
  `mouse.rs`, `multi_cursor.rs`, `line_ops.rs`, `history_ops.rs`,
  `focus_ime.rs`, `scroll_timer.rs`. `dispatch.rs` holds the top-level
  `update()` match that routes to them.
- **`canvas_impl.rs` is now `render/canvas.rs`**, and the highlighting logic it
  used to carry moved again, into `render/highlighting.rs`.

This follows the **Single Responsibility Principle** and makes the codebase maintainable.

### 4. Canvas-Based Rendering

**Location:** `canvas_editor/render/canvas.rs`

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

**Location:** `canvas_editor/editing/history.rs`

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

### 6. Generated Boilerplate for Boolean Options

**Location:** `canvas_editor/bool_options.rs` (the macro and its table),
`canvas_editor/config.rs` (the hand-written setters)

A boolean editor option is up to three methods — `set_x`, `x`, `with_x` — but
only one of them carries information. The **setter** knows what the feature is
and which cache toggling it must invalidate, so it stays hand-written in
`config.rs`. The getter and the builder are pure boilerplate: about fifty lines
apiece once the mandatory `# Returns` / `# Arguments` / `# Example` sections
are written out. Those are generated from a table:

```rust
bool_options! {
    auto_indent_enabled, set_auto_indent_enabled,
        "Returns whether auto-indentation is enabled.",
        "`true` if auto-indentation is enabled, `false` otherwise",
        default: "Enabled by default.", "";
    // ... one row per option, the builder clause optional
}
```

**Two things this buys beyond the line count**, and they are the reason it
exists rather than a preference for macros:

- **It closed a real inconsistency.** The hand-written builders were split
  between assigning the field directly and delegating to the setter, and the
  difference was accidental. A builder that assigns silently skips whatever the
  setter does besides assigning — which is a trap waiting for the next option
  to grow a cache invalidation. Every generated builder delegates.
- **The generated example *asserts* the default** rather than stating it in
  prose the way the hand-written getters did. A default that changes now fails
  a doctest instead of quietly contradicting its own documentation. That is a
  meaningful share of the 240 doctests.

The doc shape is stated once in the macro, so an option cannot arrive with a
section missing — which matters under `missing_docs = "deny"` and the project's
rule that every public item carries an example.

**When *not* to add a row:** an option whose getter does more than return the
field, or whose builder must do something the setter does not. The macro is for
the mechanical half only; anything with behavior belongs in `config.rs` beside
the setter.

## Key Implementation Details

### Syntax Highlighting

**Integration:** Uses the `syntect` crate. The optional `two-face` feature adds
extra Sublime syntax/theme definitions beyond syntect's defaults; `demo-app`
enables it. It must be declared with `default-features = false, features =
["syntect-fancy"]` — two-face defaults to `syntect-onig`, which would drag the C
`onig` backend into a workspace that builds syntect with the pure-Rust
`default-fancy` backend and targets WASM.

The token palette is not fixed: `CodeEditor::resolve_syntax` picks
`base16-ocean.light` or `base16-ocean.dark` from the lightness of the editor
`Style`'s background (`theme::is_dark_background`). Because the per-line cache
stores *resolved colors* rather than scopes, `set_theme` invalidates it. The
rainbow-bracket cycle (`CodeEditor::bracket_pair_colors`) branches on the same
predicate.

Resolution itself is memoized per editor in `ResolvedSyntax` (`caches.rs`).
`find_syntax_by_extension` is a linear scan over every bundled grammar's
extension list, and the canvas re-resolves on every frame. The memo is
*self-keyed* on the syntax identifier and the background lightness rather than
invalidated by hand, so `set_syntax`/`set_theme` cannot leave it stale.

`CodeEditor::syntax` returns the identifier the host set; `syntax_name` returns
the grammar it resolved to, including the `"Plain Text"` fallback. Status bars
want the latter.

Highlighting is **not** recomputed naïvely per frame. Instead, each logical line is
tokenized once and memoized as a dense per-line prefix that also stores the syntect
parser/highlight state left *after* the line, so multi-line constructs (block
comments, multi-line strings) resume correctly:

```rust
// render/highlighting.rs — resumes from the cached state of line N-1
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

**Location:** `canvas_editor/editing/cursor_set.rs`

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
  `editing/selection.rs` delegates to the primary cursor (see [Selection Direction](#4-selection-direction)).

### Line Wrapping (Visual Lines)

**Location:** `canvas_editor/render/wrapping.rs`

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

**Location:** `canvas_editor/features/folding/mod.rs` (region detection), `canvas_editor/features/folding/ops.rs` (fold/unfold operations), `canvas_editor/mod.rs` (state fields)

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

### Sticky Scroll

**Location:** `canvas_editor/features/sticky_scroll.rs` (which lines to pin, pure logic),
`canvas_editor/render/view.rs` (`create_sticky_scroll_layer`, `create_sticky_header_row`),
`canvas_editor/editing/cursor.rs` (`scroll_to_line`), `canvas_editor/config.rs` (toggle)

While scrolling deep inside a long block, the header lines of the enclosing
blocks stay pinned at the top of the viewport, outermost first — so the
structural context (which `impl`, which function, which `match`) never leaves
the screen. Enabled by default.

```rust
pub(crate) fn sticky_headers(
    regions: &[FoldRegion],
    top_line: usize,
    max_lines: usize,
) -> Vec<usize>; // header lines, outermost first
```

**Scope detection is shared with folding, the toggle is not.** The enclosing
blocks are exactly the [`FoldRegion`]s containing the line, so detection is the
same indentation-based scan (and carries the same trade-off: language-agnostic,
but misleading on badly indented code). What sticky scroll reads is
`block_regions()`, **not** `foldable_regions()`:

```rust
// folding/ops.rs
block_regions()     // memoized by buffer_revision — ignores folding_enabled
foldable_regions()  // returns empty when folding is disabled
```

The blocks a line sits in are a property of the buffer, not of whether the user
may collapse them. Turning code folding off therefore removes the chevrons and
leaves the pinned headers alone. Wiring sticky scroll to `foldable_regions()`
was a real bug, fixed in `6889b08`.

**A strict comparison decides what is pinned.** A region qualifies when
`start_line < top_line <= end_line`. The strictness on `start_line` matters:
when the header *is* the topmost visible line it is already on screen, and
pinning it would show the same line twice.

`DEFAULT_MAX_STICKY_LINES` (5, matching VS Code) bounds the count — without it
a deeply nested block would bury the code the reader is looking at.

**Rendering** is a widget layer, not canvas geometry: `create_sticky_scroll_layer`
stacks a `Column` of header rows over the editor, each row a `MouseArea`
publishing `Message::StickyScrollJump(line)`. Each header reuses
`highlighted_line_cached()` and the same tab expansion as the canvas text
layer, so a pinned header is visually identical to the line it mirrors. The
fold margin is deliberately left empty — a chevron there would invite a click
that does nothing.

**Clicking a header must account for the layer itself.** The sticky layer is
drawn *over* the top rows, so scrolling a line to row 0 does not make it
visible — it arrives underneath the headers that are still pinned. That is what
`sticky_headroom(line)` answers, and it is passed as `scroll_to_line`'s
`rows_above` argument. Getting this wrong lands the jump target behind the
headers, which is the bug `3b2084c` fixed.

### Search and Replace

**Location:** `canvas_editor/features/search/mod.rs` (state/matching), `canvas_editor/features/search/dialog.rs` (UI)

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

### Command Palette

**Location:** `canvas_editor/features/command_palette/mod.rs` (state + registry), `canvas_editor/features/command_palette/dialog.rs` (UI), `canvas_editor/features/actions.rs` (shared availability snapshot and shortcut hints)

`Ctrl+Shift+P` opens a filtered list of every action available right now. Its
reason to exist is discoverability: an action is reachable without knowing its
shortcut, and each row displays that shortcut, so the palette also teaches them.

```rust
pub(crate) struct CommandPaletteState {
    pub(crate) query: String,
    pub(crate) is_open: bool,
    pub(crate) selected: usize,       // index into the *filtered* list
    pub(crate) input_id: Id,
    pub(crate) scrollable_id: Id,
}

pub(crate) enum PaletteAction {
    Builtin(Box<Message>),  // re-entered through CodeEditor::update
    Custom(String),         // forwarded as Message::CommandPaletteAction(id)
}
```

**Registry.** `build_entries()` concatenates the host's entries
(`custom_command_palette_entries`, a `Vec<ContextMenuItem>`) with the built-in
commands, in that order. Host entries reuse `ContextMenuItem` rather than a
parallel type: the two surfaces describe the same thing — a stable `id`, a
`label`, a `shortcut` hint, an `enabled` flag — so an action offered in both is
declared once and keeps one identifier through one host handler.

**Availability, not dimming.** `default_entries()` takes an `ActionContext`
snapshot and only emits the commands that are usable: no `Undo` with an empty
history, no `Cut`/`Copy` without a selection, no folding commands while
`folding_enabled` is off. Host entries with `enabled: false` are dropped the
same way. This deliberately differs from the context menu, which dims them
instead — a menu with a stable shape supports muscle memory, while a search
result list should only offer runnable rows, which also keeps arrow navigation
free of unselectable stops.

**Filtering** is a case-insensitive *subsequence* match (`matches_query`), so
`tc` finds "Toggle Line Comment" and `fldall` finds "Fold All". Both sides are
folded through `char::to_lowercase`, so it works beyond ASCII. Every keystroke
resets `selected` to 0: after a filter change the row at the old index is a
different command, and keeping it would run something the user never looked at.

**Execution.** `handle_submit_command_palette_msg` closes the palette and
returns `Task::done(message)` rather than applying the action in place. The
message therefore travels back out through the host application exactly as it
would if the user had pressed the shortcut — which is what makes the actions
the editor cannot perform itself (`WriteRequested`, `RevealInFileManager`, and
every host-registered command) reach the handler that already intercepts them.

**Keyboard.** The palette's `text_input` holds focus while it is open, so
Escape would merely unfocus it and the arrow keys would move the caret inside
the query. A transparent `Canvas` layer (`KeyListener`) stacked over the dialog
captures `Escape`/`ArrowUp`/`ArrowDown` first and publishes the palette's own
messages — the same trick `goto_line/dialog.rs` uses for Escape alone. `Enter`
is the input's `on_submit`. In `escape_shortcut`, the palette is the innermost
dialog: it closes before go-to-line, which closes before search.

**Scrolling.** Rows have a fixed `ROW_HEIGHT`, which is what lets
`scroll_command_palette_to_selection` compute an absolute offset for the
highlighted row without measuring anything; the list shows `MAX_VISIBLE_ROWS`
before scrolling.

**Shared with the context menu.** `features/actions.rs` holds `ActionContext`
(the availability snapshot, built by `CodeEditor::action_context()`) and every
platform-dependent shortcut hint string. Both surfaces read from it, so a
rebinding updates one place and the two can never disagree on how an action is
spelled.

### Context Menu and Shared Actions

**Location:** `canvas_editor/features/context_menu.rs` (the menu and its public types),
`canvas_editor/features/actions.rs` (`SharedAction`, `ActionContext`, shortcut hint strings)

Right-click opens a menu of editor actions. The host extends it with its own
entries, and the same entries can appear in the
[Command Palette](#command-palette) — which is what makes the binding between
the two surfaces worth stating explicitly.

**Public types.** Both surfaces describe the same shape, so there is one type
for it rather than two parallel ones:

```rust
pub struct ContextMenuItem {
    pub id: String,             // stable identifier sent to the host
    pub label: String,          // display text
    pub shortcut: Option<String>,
    pub enabled: bool,
}

pub enum ContextMenuEntry {
    Item(ContextMenuItem),
    Separator,
}
```

`id` is what the host receives, deliberately separate from `label`: renaming or
translating the label then never breaks the action. The menu accepts
`ContextMenuEntry` (it can draw separators); the palette accepts
`ContextMenuItem` (a search result list has nothing to separate).

**One binding, two surfaces.** Undo, Redo, Cut, Copy, Paste, Select All and
Reveal in File Manager appear in both. They are declared once, as
`SharedAction`:

```rust
pub(crate) enum SharedAction { Undo, Redo, Cut, Copy, Paste, SelectAll, RevealInFileManager }

impl SharedAction {
    fn label(self, translations: &Translations) -> String;
    fn shortcut(self) -> &'static str;
    fn message(self) -> Message;
    fn is_available(self, ctx: &ActionContext) -> bool;
}
```

Each surface still spells out its **own display order**, and each keeps its own
policy for an unavailable action — the menu dims the row, the palette omits it
(see [Command Palette](#command-palette) for why they differ). What they must
not disagree on is the binding: which label goes with which shortcut hint,
which `Message` it sends, and what makes it available. Before `c196ddf` that
was written out twice, so an action could be spelled one way in the menu and
another in the palette.

**Exhaustiveness is enforced by the compiler.** `SharedAction::ALL` is
`#[cfg(test)]` — nothing in the rendering path iterates the whole set, since
each surface has its own order. It exists so that
`context_menu::tests::test_both_surfaces_render_every_shared_action_from_the_same_binding`
is exhaustive by construction: a new variant that is not added to the array
fails to compile against its declared length `[Self; 7]`. Adding a shared
action therefore cannot silently skip one surface.

**Availability** is read from `ActionContext`, a snapshot built by
`CodeEditor::action_context()` (has a selection, history is non-empty, folding
is on, …). The platform-dependent shortcut hint strings live in the same file,
`#[cfg]`-selected between `⌘Z` and `Ctrl+Z`, so a rebinding updates one place.

### Auto-Indentation

**Location:** `canvas_editor/config.rs` (toggle/style), `canvas_editor/input/update/text_input.rs` (indent-copy logic)

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

### Auto-Closing Brackets/Quotes

**Location:** `canvas_editor/input/update/text_input.rs` (logic), `canvas_editor/config.rs` (toggle)

When `auto_close_brackets` is set, `handle_character_input_msg()` branches on the typed
character before falling back to a plain `InsertCharCommand`:

```rust
fn matching_close(ch: char) -> Option<char> {
    match ch {
        '(' => Some(')'),
        '[' => Some(']'),
        '{' => Some('}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        _ => None,
    }
}
```

**Three behaviours, evaluated per cursor:**

- **Surround selection** (`surround_selections_with_pair()`): if the cursor has a
  selection and the typed char has a pair, the open char is inserted at the selection
  start and the close char at the selection end, instead of replacing the selection.
  The wrapped text stays selected — the cursor's `anchor`/`position` are recomputed to
  keep the original selection direction.
- **Type-through**: if the typed char is a closing char and it already sits immediately
  after the cursor (`char_at(pos) == Some(ch)`), the cursor just steps over it — no
  buffer mutation, no history entry.
- **Auto-close insert** (`insert_pair_at_cursor()`): if the typed char opens a pair and
  `should_auto_close()` says the following character is EOL, whitespace, or another
  closing bracket/quote (so it never wraps mid-identifier, e.g. typing `'` inside
  `sn|ake`), both characters are inserted with the cursor left between them.

Multi-cursor adjustment reuses `EditType::InsertChar` — since a 2-character insert at
column `c` is equivalent to two 1-character inserts at the same `c`,
`adjust_other_cursors()` is simply called twice instead of adding a new `EditType`
variant (see [Multi-Cursor Edit Order](#5-multi-cursor-edit-order)).

`set_auto_close_brackets()` toggles the whole feature; the demo app exposes it as a
toolbar checkbox next to "Auto-indentation".

### Matching Bracket/Quote Highlight

**Location:** `canvas_editor/features/bracket_match.rs` (detection), `canvas_editor/render/overlays.rs` (overlay draw), `canvas_editor/config.rs` (toggle)

When `bracket_match_highlight_enabled` is set, `draw_matching_bracket_highlight()` calls
`find_matching_pair()` with the primary cursor's position on every overlay redraw:

```rust
pub(crate) fn find_matching_pair(
    buffer: &TextBuffer,
    pos: (usize, usize),
) -> Option<((usize, usize), (usize, usize))>;
```

**Detection, per character class:**

- **Brackets** (`( ) [ ] { }`): the cursor must touch a bracket (immediately before or
  after it). The buffer is then scanned forward (for an opener) or backward (for a
  closer), tracking a same-family nesting depth so a `(` inside a `[]` pair is skipped
  while looking for a `)`.
- **Quotes** (`" '`): since a quote doesn't distinguish opener from closer, all quotes
  of that kind on the *same line* are paired sequentially in the order they appear
  (1st with 2nd, 3rd with 4th, ...). The pair containing the cursor's quote is returned.

Both scans are plain textual scans — like [Auto-Closing Brackets/Quotes](#auto-closing-bracketsquotes),
they don't skip over brackets/quotes found inside strings or comments, and quote
matching doesn't account for escaped quotes (e.g. `\"`).

**Rendering:** the two positions returned by `find_matching_pair()` are each converted
to a visual line via `WrappingCalculator::logical_to_visual()` and drawn as a 1-char-wide
rectangle in the `overlay_cache` layer, reusing `fill_highlight_segment()` (the same
helper used for search-match highlights). No dedicated cache-invalidation is needed:
`overlay_cache.clear()` already runs on every cursor move and edit, so the pair is
recomputed fresh on the next relevant redraw.

`set_bracket_match_highlight_enabled()` toggles the whole feature (and clears
`overlay_cache` immediately so the change is visible without waiting for the next
cursor move); the demo app exposes it as a toolbar checkbox next to "Show whitespace".

### Bracket-Pair Colorization

**Location:** `canvas_editor/features/bracket_match.rs` (depth logic), `canvas_editor/caches.rs`
(`BracketDepthCache`), `canvas_editor/config.rs` (toggle), `canvas_editor/render/text.rs` (draw pass),
`canvas_editor/render/highlighting.rs` (palettes, `bracket_pair_colors()`)

Unlike [Matching Bracket/Quote Highlight](#matching-bracketquote-highlight), which
only reacts to the cursor, this feature colors **every** `( ) [ ] { }` in the visible
text by its nesting depth, so a matching pair always shares a color:

```rust
// bracket_match.rs
pub(crate) fn bracket_depth_indices(
    line: &str,
    start_depth: usize,
) -> Vec<(usize, usize)>; // (column, palette depth index)

pub(crate) fn bracket_depth_after_line(line: &str, start_depth: usize) -> usize;
```

For an opener the returned index *is* the depth entering it, then depth increases;
for a closer, depth decreases first and the returned index is the result — so `(` and
its `)` always compute the same index. Depth saturates at `0` on an unbalanced closer
instead of underflowing. Like the other bracket helpers, this is a plain textual
scan with no string/comment awareness.

**Per-line depth cache:** Knowing the color of a bracket also requires knowing the
nesting depth *entering* its line, which depends on every bracket before it in the
file. `BracketDepthCache` (`mod.rs`) memoizes this as a dense prefix — `depths[i]` is
the depth entering logical line `i` — mirroring `HighlightCache`'s sequential
extend/truncate shape, but without any syntect state (bracket counting is a cheap
plain-text scan, so there's no need for `HIGHLIGHT_LINES_PER_FRAME`-style budget
throttling):

```rust
pub(crate) fn depth_at_line_start(
    &mut self,
    buffer: &TextBuffer,
    line: usize,
) -> usize; // extends the prefix as needed, then returns depths[line]

pub(crate) fn truncate_from(&mut self, line: usize); // keeps depths[0..=line]
```

`finish_edit_operation()` truncates it from `pre_edit_line - 1` right alongside
`invalidate_highlight_from()` (see [Highlight Cache Anchor](#7-highlight-cache-anchor-pre_edit_line)),
and `reset()` replaces it wholesale on full content swap.

**Rendering:** `draw_bracket_pair_colors()` runs in the `content_cache` layer (not
`overlay_cache` — like syntax highlighting, this depends on buffer content, not
cursor/selection state), right after `draw_text_with_syntax_highlighting()` for each
visual line. Rather than threading override colors through the token-splitting
loop (which would have to replicate tab-expansion math), it redraws just the bracket
characters *on top of* the already-rendered glyphs, at the same pixel position
computed by `calculate_segment_geometry()` — the same geometry helper selection/search
highlighting uses:

```rust
// render/highlighting.rs — one cycle per background lightness
const BRACKET_PAIR_COLORS_DARK: [Color; 3] = [
    /* gold */ Color { r: 1.0, g: 0.843, b: 0.0, a: 1.0 },
    /* orchid */ Color { r: 0.855, g: 0.439, b: 0.839, a: 1.0 },
    /* light sky blue */ Color { r: 0.529, g: 0.808, b: 0.980, a: 1.0 },
];

const BRACKET_PAIR_COLORS_LIGHT: [Color; 3] = [
    /* blue */ Color { r: 0.016, g: 0.192, b: 0.980, a: 1.0 },
    /* green */ Color { r: 0.192, g: 0.576, b: 0.192, a: 1.0 },
    /* brown */ Color { r: 0.482, g: 0.220, b: 0.078, a: 1.0 },
];
```

**Two palettes, not one.** `bracket_pair_colors()` picks the cycle from the
lightness of the style's background (`theme::is_dark_background`), exactly as
`resolve_syntax` picks the syntect theme. The dark cycle is VS Code's Dark+
rainbow (gold / orchid / light sky blue); reusing those bright, low-contrast
hues on a white background leaves brackets barely visible, so the light cycle
is its own saturated, dark-toned triple following VS Code Light+.

`set_bracket_pair_colorization_enabled()`
toggles the whole feature and clears `content_cache`; the demo app exposes it as a
toolbar checkbox ("Rainbow brackets") next to "Highlight matching bracket".

### Indentation Guides

**Location:** `canvas_editor/features/indent_guides.rs` (level computation, pure logic),
`canvas_editor/metrics.rs` (`indent_width`), `canvas_editor/render/text.rs` (draw pass),
`canvas_editor/config.rs` (toggle), `theme.rs` (`Style::indent_guide_color`)

A thin vertical line is drawn at every indentation level, so nesting is visible without
following braces. The feature splits cleanly in two: *how many guides does this line
deserve* (pure, unit-tested) and *where do they go on screen* (rendering).

`guide_levels(buffer, line, unit) -> usize` answers the first question. For a non-blank
line it is simply `indent_width(line) / unit`. `indent_width` lives in `metrics.rs`
alongside `TAB_WIDTH` and `measure_char_width` — it returns a width in **display
columns**, not character indices, so a tab counts as `TAB_WIDTH` columns exactly as it
does when rendering. Code folding uses the same function for its region detection.

Blank lines have no indentation of their own, so `guide_levels` infers theirs from the
nearest non-blank line above and below and takes the **smaller** of the two:

```text
fn f() {
│   a();
│              <- blank, min(1, 1) = 1 guide: stays inside the block
│   b();
}
```

```text
fn f() {
│   a();
               <- blank, min(1, 0) = 0 guides: the block is already closed below
}
```

Both scans are bounded by `MAX_BLANK_RUN_SCAN` (200 lines). Without it, a buffer made
mostly of blank lines would turn every visible line into a full-buffer scan; past that
many blank lines the run is treated as a gap between blocks and no guide is drawn.

`draw_indent_guides()` runs in the **content layer**, next to `draw_bracket_pair_colors()`.
Since indentation is ASCII whitespace, the x position is plain arithmetic on display
columns rather than a `calculate_segment_geometry()` call (which reasons in character
indices):

```rust
let x = ctx.gutter_width + 5.0 - ctx.horizontal_scroll_offset
    + (level * unit) as f32 * ctx.char_width;
```

Two constraints shape the draw pass:

- **Guides are skipped on wrapped continuation segments** (`visual_line.is_first_segment()`).
  Every visual line starts drawing at the same base X, so a guide placed at its original
  column would land on top of the wrapped text instead of in its indentation.
- **No z-order handling is needed.** Iced draws all geometry below all text within a
  frame, so a guide filled with `fill_rectangle` automatically sits behind the glyphs.

`unit` comes from the editor's `IndentStyle` (`Spaces(n)` → `n`, `Tab` → `TAB_WIDTH`),
so changing the indent style moves the guides with it. `unit == 0` draws nothing.

`set_show_indent_guides()` toggles the feature and clears `content_cache`; the demo app
exposes it as `EditorToggle::IndentGuides` ("Show indentation guides") in the editor
options panel.

### Color Preview Swatches

**Location:** `canvas_editor/features/color_preview.rs` (detection, pure logic),
`canvas_editor/render/text.rs` (swatch drawing), `canvas_editor/config.rs` (toggle)

A small colored square is drawn next to a color literal — `#1e1e2e`,
`0xFF6B6B`, `rgb(58, 123, 213)`, `rgba(…)` — so the color can be read at a
glance instead of decoded mentally.

```rust
pub(crate) struct ColorLiteral {
    pub start_col: usize,
    pub end_col: usize,
    pub color: Color,
}

pub(crate) fn color_literals(line: &str) -> Vec<ColorLiteral>;
```

**Detection is purely lexical.** No syntax awareness is involved, so a literal
inside a comment or a string is reported like one in real code — which is what
a reader scanning for colors expects, and keeps the scan independent of the
highlight cache. `starts_new_token()` guards the left edge so the `#1e1e2e`
inside a longer identifier is not matched, and the functional forms are bounded
by a maximum scan length rather than running to end of line.

Short hex is expanded (`#abc` → `#aabbcc`), and both `#`- and `0x`-prefixed
forms are accepted, with optional alpha. Channels parse as either `0-255` or a
percentage; alpha as a ratio.

**Rendering** happens in the `content_cache` layer alongside the other
content-derived decorations, since the swatch depends on buffer text, not on
cursor state. The swatch is sized as a fraction of the line height, so it
follows the font size, and framed with a thin border so a swatch matching the
editor background stays visible.

### Vim Emulation

**Location:** `canvas_editor/features/vim/mod.rs` (modal state and the key-sequence
parser), `canvas_editor/features/vim/update.rs` (applying the parsed commands),
`canvas_editor/tests/` (three integration suites)

Opt-in Vim-style modal editing, off by default. `CodeEditor::vim_mode()` returns
`Option<VimMode>` — `None` while Vim is disabled — so a status bar can
distinguish "Vim is off" from "Vim is in Normal mode".

```rust
pub enum VimMode { Normal, Insert, Visual, VisualLine }
```

**The parser is a small state machine, not a key map.** A Vim command is a
sequence, so recognizing one needs state between keystrokes. `VimState` holds
exactly what a partially typed command needs:

```rust
pub(crate) struct VimState {
    mode: VimMode,
    count: Option<usize>,             // the `3` of `3dw`
    g_prefix: bool,                   // `g` typed, waiting for `g`/`e`/...
    pending_operator: Option<VimOperator>,   // `d` typed, waiting for a motion
    pending_operator_count: usize,
    visual_anchor: Option<(usize, usize)>,   // where visual mode started
    visual_active: Option<(usize, usize)>,
    command_line: Option<VimCommandLine>,    // `:` / `/` line being typed
    last_search: Option<String>,             // for `n` / `N`
    register: VimRegister,                   // yank/delete register
}
```

Keys are parsed into three vocabularies — `VimMotion` (`h j k l w b e 0 ^ $ gg
G` …), `VimOperator` (`d c y` …) and `VimAction` (everything that is neither) —
which is what lets an operator compose with any motion instead of enumerating
every pair.

**Integration with the editor's own undo grouping** is the subtle part:
`keep_vim_insert_group()` (`input/update/mod.rs`) keeps a typing group open
while Vim is in Insert mode, so a whole insert session undoes as one step —
the Vim expectation — rather than following the editor's normal navigation
boundaries. See [Command History Grouping](#3-command-history-grouping).

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

**Location:** `canvas_editor/focus.rs` (request/query/lose focus), `canvas_editor/input/events.rs` (`has_focus()`, keyboard gating), `canvas_editor/mod.rs` (statics, `editor_id` field)

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

**Scrolling is measured in visual rows, never logical lines.** With wrapping
enabled a logical line occupies several rows, so multiplying the logical line
index by the row height would scroll to the wrong place. `scroll_to_cursor()`
maps the primary cursor through `logical_to_visual()` first, and multiplies by
the `line_height` **field** (which follows the configured font size), not the
`LINE_HEIGHT` constant:

```rust
pub(crate) fn scroll_to_cursor(&self) -> Task<Message> {
    let visual_lines = self.visual_lines_cached(self.viewport_width);
    let pos = self.cursors.primary_position();

    // Fall back to the logical line only if the position has no visual row
    // (e.g. it sits inside a collapsed fold).
    let cursor_y = WrappingCalculator::logical_to_visual(&visual_lines, pos.0, pos.1)
        .map_or(pos.0, |visual_idx| visual_idx) as f32
        * self.line_height;

    if cursor_y < viewport_top + top_margin {
        // Scroll up
    } else if cursor_y + self.line_height > viewport_bottom - bottom_margin {
        // Scroll down
    }

    scroll_to(self.scrollable_id.clone(), AbsoluteOffset { y: new_scroll })
}
```

The same rule governs paging: `page_up()` / `page_down()` move by
`viewport_height / line_height` **visual rows**, not logical lines
(`editing/cursor.rs`). Any new viewport arithmetic belongs in visual-row space.

**Smart margins:** 2 rows of padding to prevent cursor at edge

### Internationalization (i18n)

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

**Location:** `canvas_editor/metrics.rs`, `canvas_editor/input/ime_requester.rs`, `canvas_editor/render/canvas.rs`

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

**Width calculation** (`canvas_editor/metrics.rs`):

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

The `measure_char_width()` function classifies characters and returns appropriate width (`canvas_editor/metrics.rs`):

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

**Location:** `canvas_editor/input/ime_requester.rs`

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

**Cursor positioning** (`canvas_editor/editing/cursor.rs`):

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

**Selection rendering** (`canvas_editor/render/text.rs`):

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

**Location:** `canvas_editor/lsp/mod.rs`, `canvas_editor/lsp/process/`

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
│  │LspProcessClient│ (lsp/process/mod.rs)            │
│  │ stdio subprocess│                                │
│  └──────────────┘                                   │
└─────────────────────────────────────────────────────┘
```

### Layer 1 — `LspClient` trait (`canvas_editor/lsp/mod.rs`)

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
    fn request_formatting(&mut self, document: &LspDocument, options: LspFormattingOptions);
}
```

`CodeEditor` holds an `Option<Box<dyn LspClient>>` and calls the trait methods automatically when the document changes or the user requests hover/completion.

### Layer 2 — `LspProcessClient` (`canvas_editor/lsp/process/mod.rs`)

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
    Formatting { uri: String, edits: Vec<LspTextChange> },
    Progress { token, server_key, title, message, percentage, done },
    Log { server_key, message },
}
```

Server configurations (command, arguments, language IDs) live in `lsp/process/config.rs` and are keyed by a short string such as `"lua-language-server"` or `"rust-analyzer"`.

**UTF-16 conversion:** LSP uses UTF-16 character offsets while the editor works in UTF-8. `TextModel` inside `LspProcessClient` mirrors the document content and converts positions before every request (`to_utf16_position`), and back again for replies whose payload is in document coordinates (`to_char_position`, used by the formatting response). A pending request therefore records the document URI alongside its kind: a JSON-RPC response carries only the request id, so the mirror to translate against can only be found through that.

### Layer 3 — `LspOverlayState` + `view_lsp_overlay` (`canvas_editor/lsp/process/overlay.rs`)

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

### Applying server edits (`canvas_editor/lsp/edits.rs`)

`CodeEditor::apply_lsp_text_edits` is where a `TextEdit[]` reply reaches the buffer — formatting today, code actions and rename later, which is why it takes a plain `&[LspTextChange]` rather than anything formatting-specific.

The batch becomes one `CompositeCommand` (a `DeleteRangeCommand` plus an `InsertTextCommand` per edit) pushed as a **single undo step**, and:

- edits are applied **last-first**, because every range refers to the document as it stands *before* any of them is applied;
- positions are clamped onto the buffer, with a line past the last one folding onto the *end* of the last line — the shape a whole-document format reply uses;
- the cursor keeps its `(line, column)`, clamped, and any selection is dropped;
- an **overlapping** batch is refused outright (returns `false`, buffer untouched): LSP forbids it, and applying one would silently corrupt the text.

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

User saves file   →  App calls editor.lsp_request_formatting()
                  →  LspProcessClient flushes didChange, sends formatting request
                  →  Server replies → LspEvent::Formatting { uri, edits }
                  →  App calls editor.apply_lsp_text_edits(&edits)
                  →  App writes the formatted file to disk
                  (a server that does not answer in time is given up on,
                   and the file is written unformatted)
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

Lines live around a movable gap (see [TextBuffer](#2-textbuffer-buffermodrs)).

**Costs:**

- O(1) line access — index lookup into one of the two vectors.
- O(1) line insert/remove **once the gap is at the edit site**: push/pop on a
  `Vec` end, no tail shifting.
- O(k) to move the gap `k` lines, paid only when the edit jumps to a different
  part of the file. Consecutive typing does not pay it.
- O(n) for inserting inside a line, where n is the *line* length — a `String`
  splice, unaffected by file size.

**Sufficient for:**

- Files well past ~10,000 lines for the usual editing patterns
- Typing, deleting, line moves, multi-cursor edits clustered together

**Not optimal for:**

- Edits that alternate between distant regions, each dragging the gap across
  the file
- Real-time collaborative editing

**Potential improvements:**

- Rope data structure for O(log n) operations regardless of locality
- Piece table for large-file handling with cheap undo of whole regions

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

### The three levels

Testing is layered, and the layers answer different questions. A change is not
covered because *some* test touches it — it is covered when the test that would
notice the regression exists at the right level.

| Level | Where | Count | Answers |
|---|---|---|---|
| Unit | `#[cfg(test)] mod tests` beside the code | 688 | Is this function correct? |
| Doctest | `# Example` blocks on public items | 240 | Is the documented usage real and still compiling? |
| Handler | `demo-app/src/{app,ui}` tests | 70 | Does `update()` do the right thing with this `Message`? |
| Interface | `demo-app/src/ui_tests/` | 74 | Does any widget actually *emit* that `Message`? |
| Integration | `iced-code-editor/tests/` | 13 | Does the public API work from outside the crate? |

### Unit Tests

Each module carries its own tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_char() { ... }
}
```

**Coverage:**

- `buffer/mod.rs`: buffer operations, gap movement, line-ending round-trip
- `editing/command/`: all command types and undo/redo
- `editing/cursor.rs`: cursor movement and paging edge cases
- `editing/selection.rs`: selection normalization and extraction
- `input/update/`: message handling and state transitions
- `theme.rs`: all Iced themes, color adaptation, helper functions

Doctests are not decoration here: `bool_options.rs` generates each boolean
option's example so that it **asserts** the documented default. A default that
changes fails a doctest instead of quietly contradicting its own documentation.

### Interface Tests (`demo-app/src/ui_tests/`)

The handler-level tests call `DemoApp::update` with a hand-built `Message`.
That proves the handler is correct, but says nothing about whether any widget
actually emits that message — a shortcut removed from `input/events.rs` would
leave every handler test green.

`ui_tests/` closes that gap. It renders the real `ui::view` widget tree in
Iced's headless [`Simulator`](https://docs.rs/iced_test) (`iced_test = "0.14"`),
clicks and types on the actual widgets, feeds the resulting messages back into
`update`, and asserts both on the state and on what the next render shows:

- `chrome.rs` (11) — toolbar, status bar, tabs
- `dialogs.rs` (23) — search, go-to-line, command palette, context menu
- `editing.rs` (37) — typing, selection, clipboard, navigation keys
- `sticky_scroll.rs` (3) — pinned headers and click-to-jump

**Known scope limit:** the simulator sees the widget tree, never
`DemoApp::subscription`. Shortcuts routed through the global event stream (the
Escape handling in `app.rs`, for instance) are covered by `update`-level tests
instead. This is also what makes the interface tests the right place to prove
that a key is *not* captured — a combination the editor declines falls through
uncaptured, which is exactly what the widget tree can observe.

### Integration Tests

`iced-code-editor/tests/` exercises the public API from outside the crate, so
anything it touches is genuinely reachable by a host application:

- `vim_command_line.rs` (6), `vim_counted_commands.rs` (4),
  `vim_toggle_shortcut.rs` (3)

`simple-example/` is not a test, but it fails the build if the minimal
embedding stops compiling, which serves a similar purpose.

### Regression tests must be verified failing

A test written alongside a bug fix proves nothing until it has been seen to
**fail against the pre-fix code**. A regression test that passes both before
and after the fix is testing something other than the bug.

The practice: stash the fix (or check out the parent commit), run the new test,
confirm it fails *and that it fails for the stated reason* — the assertion
message should name the wrong value, not a panic from unrelated drift — then
restore the fix and confirm it passes. Recent examples: paging landed on
`(3, 0)` instead of `(0, 84)` before the wrap fix; `Ctrl+Page Down` moved the
cursor to line 30 instead of leaving it at 0 before the host-passthrough fix.

### Running Tests

```bash
# Run all tests: 688 unit + 144 demo-app + 13 integration + 240 doctests
cargo test --workspace --all-features

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_insert_char

# Interface tests only
cargo test -p demo-app ui_tests

# Coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace --all-features --summary-only
cargo llvm-cov --workspace --all-features --html
```

### Benchmarks

Performance-critical hot paths are benchmarked with [criterion](https://github.com/bheisler/criterion.rs). The benchmarks live in `iced-code-editor/benches/editor_benchmarks.rs` and measure the work performed per edit / per scroll on a synthetic 10,000-line source file:

| Benchmark | Function under test |
|---|---|
| `highlight_line_spans` | Tokenizing one line into colored spans |
| `calculate_visual_lines_10k` | Line wrapping (`WrappingCalculator`) |
| `compute_foldable_regions_10k` | Fold-region detection |
| `find_matches_10k` | Search across the buffer |

**Feature gate:** These functions are internal, so they are exposed to the benchmark crate through the hidden `bench_support` module (`canvas_editor/bench_support.rs`, re-exported from `lib.rs`). Both the module and the `[[bench]]` target are gated behind the `bench` feature (`required-features = ["bench"]`), so the benchmarks are invisible to normal builds and to the public API.

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

**Problem:** Rust strings are UTF-8, so byte indices ≠ character indices. Slicing
a string with a character offset panics on a non-`char` boundary (accents, CJK,
emoji).

**Solution:** Use the char-aware helpers centralized in `text_utils.rs`, never
slice with a raw column offset. Every slicing site (text buffer, selection,
canvas rendering, LSP) goes through these:

```rust
// text_utils.rs — single boundary
pub(crate) fn char_to_byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices().nth(char_index).map_or(s.len(), |(idx, _)| idx)
}

// text_utils.rs — both boundaries of a [start, end) range in one pass.
// Prefer this over two char_to_byte_index() calls: O(end_char) instead of
// O(n) per boundary. Used in the highlight/selection hot paths.
pub(crate) fn char_range_to_byte_range(
    text: &str,
    start_char: usize,
    end_char: usize,
) -> (usize, usize) { /* ... */ }
```

### 2. Cache Invalidation

**Problem:** Forgetting to clear a cache leaves stale rendering — but clearing the
*wrong* layer is the subtler trap. Rendering is split across two `canvas::Cache`
layers (see [Canvas-Based Rendering](#4-canvas-based-rendering)):

- `content_cache` — syntax-highlighted text and the gutter. Expensive to rebuild.
- `overlay_cache` — cursor, current-line highlight, selection, search matches, IME.

Clearing `content_cache` on every cursor blink or drag would destroy rendering
performance; clearing only `overlay_cache` after a buffer/layout change would leave
stale text on screen.

**Solution:** Clear the layer that actually changed.

```rust
// Cursor / selection / search moved → overlay only
self.cursors.set_single(new_position);
self.overlay_cache.clear();

// Buffer / syntax / theme / wrap / fold changed → both layers
self.content_cache.clear();
self.overlay_cache.clear();
```

### 3. Command History Grouping

**Problem:** Forgetting to end groups leaves consecutive operations merged into a single undo step (broken undo boundaries)

**Solution:** Always pair the start and end of a group. The grouping logic is
encapsulated in two helpers in `input/update/mod.rs` that guard on the
`is_grouping` flag:

```rust
// Begin grouping on the first edit of a typing run
fn ensure_grouping_started(&mut self) {
    if !self.is_grouping {
        self.history.begin_group();
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

### 5. Multi-Cursor Edit Order

**Problem:** When the same edit is applied at several cursors, applying it in
document order corrupts every cursor below the first edit. Inserting a character
at an upper position shifts all positions after it, so a cursor stored as
`(line, col)` no longer points where it should — subsequent inserts land at the
wrong column (or the wrong line, after a newline/merge).

**Solution:** Always apply multi-cursor edits in **descending document order**, so
that edits at higher positions never invalidate positions still to be processed.
Every edit handler asks the cursor set for that order before iterating. The
sort is **not** rewritten per handler — `CursorSet::descending_order()`
(`editing/cursor_set.rs`) is the single implementation:

```rust
// input/update/text_input.rs — handle_character_input_msg, handle_tab;
// input/update/deletion.rs — the delete handlers; and so on.
let order = self.cursors.descending_order();
for &idx in &order {
    // apply edit at cursor `idx`, then fix the *other* cursors:
    adjust_other_cursors(self.cursors.as_mut_slice(), idx, line, col, edit_type);
}
```

`descending_order_by_key(filter, key)` is the general form, used when the order
must follow something other than the cursor position — `surround_selections_with_pair()`
sorts by *selection start* — and when cursors without a selection must be
skipped entirely.

`adjust_other_cursors()` (`input/update/mod.rs`) shifts the remaining cursors' positions and
selection anchors for the edit just made, and `sort_and_merge()` collapses any
cursors that end up overlapping. A new edit handler that iterates cursors in their
natural order, or that forgets `adjust_other_cursors()`, will work with a single
cursor but silently corrupt multi-cursor edits.

### 6. Buffer Revision Bumping

**Problem:** Not every cache is *cleared* — several are **memoized by revision** and
only recomputed when their key changes:

- `visual_lines_cache` — line wrapping (keyed by `buffer_revision`, viewport, wrap/fold)
- `foldable_regions_cache` — fold detection (keyed by `buffer_revision`)
- `max_content_width_cache` — horizontal scroll extent (keyed by `buffer_revision`)

Mutating the buffer without changing `buffer_revision` leaves all of these serving
stale layout — wrapping, fold regions and scroll width computed against the *old*
text. This is distinct from [Cache Invalidation](#2-cache-invalidation), which only
concerns the two `canvas::Cache` rendering layers.

**Solution:** Route every buffer mutation through `finish_edit_operation()`, which
bumps the revision (and clears the canvas caches and the highlight prefix):

```rust
// input/update/mod.rs — finish_edit_operation()
self.buffer_revision = self.buffer_revision.wrapping_add(1);
*self.visual_lines_cache.borrow_mut() = None;
self.invalidate_highlight_from(self.pre_edit_line.saturating_sub(1));
self.bracket_depth_cache.borrow_mut()
    .truncate_from(self.pre_edit_line.saturating_sub(1));
self.content_cache.clear();
self.overlay_cache.clear();
```

Fold-state changes have their own counter (`fold_revision`, bumped via
`bump_fold_revision()`); the visual-lines cache key includes both. The exact values
are not meaningful — `wrapping_add` is used so overflow is harmless.

### 7. Highlight Cache Anchor (`pre_edit_line`)

**Problem:** The syntax-highlight prefix is not cleared on edit; it is **truncated**
from `pre_edit_line` so only lines at or below the edit are re-tokenized
(see [Syntax Highlighting Optimization](#2-syntax-highlighting-optimization)). If
`pre_edit_line` is left pointing higher than the real edit, work is wasted; if it is
left pointing *below* the edit, lines above the truncation keep stale colors — a
visible bug for multi-line constructs (block comments, strings) whose resume state
changed.

**Solution:** Capture `pre_edit_line` *before* the edit, from the topmost active
line, and reset it for edits not anchored to one line:

```rust
// input/update/dispatch.rs — captured at the top of update() before dispatching
self.pre_edit_line = self.min_active_line();

// Operations whose effect is not local to one line reset the anchor to 0
// so the whole prefix is rebuilt: undo, redo, Replace All.
self.pre_edit_line = 0;
```

`min_active_line()` returns the smallest line touched by any cursor or selection
anchor. `finish_edit_operation()` then truncates from `pre_edit_line - 1` (a
one-line margin covering edits that merge with the preceding line, e.g. backspace at
column 0). A new edit handler must keep `pre_edit_line` consistent with where it
actually mutates the buffer.

### 8. InsertTextCommand Cursor Override vs. Undo

**Problem:** `InsertTextCommand::with_cursor_after()` (`editing/command/edit.rs`) lets a command
override where the cursor rests after `execute()` — used by auto-close bracket
insertion to leave the cursor *between* the two inserted characters instead of after
them. But `InsertTextCommand::undo()` assumes the opposite: it walks backward from
`cursor_after`, deleting one character per step via `buffer.delete_char()` (a
backspace-style delete of the character *before* the given column). That walk is only
correct when `cursor_after` sits *after* the whole inserted string. Overriding it to a
position *inside* the string (e.g. between `(` and `)`) makes undo delete the wrong
character and leave one of the pair behind.

**Solution:** When the resting cursor must land inside the inserted text, don't use a
single `InsertTextCommand` with an overridden cursor. Push one `InsertCharCommand` per
character instead (as `insert_pair_at_cursor()` and `surround_selections_with_pair()`
in `input/update/text_input.rs` do) — each command's own `cursor_before`/`cursor_after` stays consistent
with what it actually inserted, so undoing them in reverse (via the history group)
restores the exact prior state regardless of where the visible cursor ends up.

```rust
// input/update/text_input.rs — insert_pair_at_cursor(): two commands, not one
// InsertTextCommand::with_cursor_after() spanning both chars.
let mut open_cmd = InsertCharCommand::new(pos.0, pos.1, open, pos);
open_cmd.execute(&mut self.buffer, &mut cursor_pos);
self.history.push(Box::new(open_cmd));

let mut close_cmd = InsertCharCommand::new(pos.0, pos.1 + 1, close, cursor_pos);
close_cmd.execute(&mut self.buffer, &mut cursor_pos);
self.history.push(Box::new(close_cmd));
```

`with_cursor_after()` remains correct and safe for its existing use (Vim paste), where
the resting cursor is still at a string boundary (start or end), never mid-string.

## Future Enhancements

Check [TODO.md](https://github.com/LuDog71FR/iced-code-editor/blob/main/TODO.md) for details.

## Contributing Guidelines

### Code Style

The rules below are not style preferences — most are enforced by the compiler
through `[workspace.lints]` in the root `Cargo.toml`, and every workspace
member opts in with `[lints] workspace = true`.

- **Rust 2024 edition.** No async runtime: concurrency goes through
  `iced::Task`, and the LSP client uses plain `std::thread` +
  `std::sync::mpsc`.
- **`unsafe_code = "forbid"`** and **`missing_docs = "deny"`**. Every public
  item needs a doc comment; the lint enforces the rule instead of review
  catching it.
- **28 clippy lints denied**, including `unwrap_used`, `expect_used`, `panic`,
  `unreachable`, `float_cmp`, `print_stdout`, `print_stderr`, `dbg_macro`,
  `needless_pass_by_value`, `missing_panics_doc` and `missing_errors_doc`.
  A clippy warning is a build failure, not a suggestion.
- **Never `#[allow(dead_code)]`.** If something is unused, either it is
  reachable and needs a caller, or it should be deleted.
- **Recover from mutex poisoning** with
  `.lock().unwrap_or_else(|error| error.into_inner())`. A plain
  `.lock().unwrap()` turns one panic into a permanently dead editor.
- **`cargo fmt`** before committing.
- **Every new function gets a unit test**, and modifying a function means
  checking and updating the tests that cover it. Frontend display functions are
  the one exception — they are covered at the
  [interface level](#interface-tests-demo-appsrcui_tests) instead.
- **Every public item gets an `# Example`** that actually runs. Doctests are
  part of the suite, not decoration.

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

Where `<scope>` is optional and names the affected area (e.g., `cursor`,
`render`, `lsp`, `deps`).

**Types**, with examples from this repository's own history:

- `feat` - New feature (`feat: display syntax name of the highlighter`)
- `fix` - Bug fix (`fix: page up/down move by one screenful when lines wrap`)
- `docs` - Documentation only (`docs: give every publicly reachable function a running example`)
- `style` - Code style/formatting (`style: apply rustfmt changes`)
- `refactor` - Code refactoring (`refactor: split keyboard shortcut recognition out of input/events.rs`)
- `perf` - Performance improvement (`perf: improve text buffer and search updates`)
- `test` - Add or modify tests (`test: split ui_tests.rs into a module per interface area`)
- `build` - Build system changes (`build(deps): bump bytes in the cargo group`)
- `ci` - CI configuration (`ci: make the security audit fail on the class it was passing`)
- `chore` - Maintenance tasks (`chore: update dependencies`)

**Breaking changes:** Add `!` after type/scope
(`fix!: bound the LspEvent queue, dropping events instead of blocking the reader`).
A breaking change must also be called out in `CHANGELOG.md` under a
`**BREAKING**` entry, saying what the caller has to change.

**Write the subject as a claim about behaviour, not a label for a diff.**
`fix: navigation closes the undo group, Home/End included` says what is now
true; `fix: update navigation.rs` does not. The body is where the reasoning
goes: what was wrong, why the fix is the right shape, and what was verified.

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
