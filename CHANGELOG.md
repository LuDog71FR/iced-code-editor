# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- feat: Criterion benchmark suite for performance-critical paths

### Changed

- perf: **Faster syntax highlighting**
  - Each line is highlighted once and cached, then reused across frames, scrolling and wrapped segments; an edit only re-highlights from the changed line onward
  - Optimized release profile (fat LTO, single codegen unit)
  - Removed `O(n)` character-to-byte conversions from the text rendering loop

- docs: Improve DEV.md
  - Add missing functionalities: multi-cursor, line wrapping, code folding, search & replace and auto-indentation
  - Fix divergences between code and documentation

- refactor: Consolidate UTF-8 char/byte conversion helpers into a shared `text_utils` module

- refactor: Deduplicate selection/search highlight rendering and page up/down cursor movement into shared helpers

### Fixed

- fix: Multi-line block comments and strings are now highlighted correctly across their entire span

## [0.3.9] - 2026-06-02

### Added

- feat: add optional dependency 'two-face' for additional syntaxes ([#18](https://github.com/LuDog71FR/iced-code-editor/issues/18))

- feat: **Code folding** for better code navigation
  - Collapse and expand code blocks with a single click or shortcut
  - Collapse and expand all code blocks with shortcuts

## [0.3.8] - 2026-04-11

### Added

- feat: **Multiple cursors** for simultaneous editing at multiple positions

- feat: **auto-indentation on Enter**: copies the leading whitespace of the current line to the new line
  - Toggle on/off via `set_auto_indent_enabled()` / `auto_indent_enabled()`

- feat: configurable indentation style via `IndentStyle` enum
  - `IndentStyle::Spaces(n)` (2, 4 or 8 spaces) or `IndentStyle::Tab`
  - Configure via `set_indent_style()` / `indent_style()`
  - Default: 4 spaces

### Fixed

- fix: Tab key was navigating to the next widget instead of inserting indentation
- fix: Example in README.md have error with focus and Input widget ([#17](https://github.com/LuDog71FR/iced-code-editor/issues/17))

## [0.3.7] - 2026-03-09

### Added

- feat: Handle horizontal scrolling when line wrapping is disabled ([#13](https://github.com/LuDog71FR/iced-code-editor/issues/13))
- feat: Language Server Protocol (LSP) support

## [0.3.6] - 2026-02-25

### Added

- feat: Handle focus without the needs to check if mouse is out of bounds ([#10](https://github.com/LuDog71FR/iced-code-editor/issues/10))
- feat: WASM compatibility optimization
- feat: Improve selection smoothness via layered canvas caching


## [0.3.4] - 2026-01-28

### Added

- feat: automatic syntax highlighting for all file extensions supported by syntect

### Fixed

- fix: crashing when searching for "a" in a file with 99,000 or more entries and only 110,000 matches
- fix: lag when performing a full replacement on a file with 100,000 or more entries
- fix: crashing when searching for Chinese characters
- fix: text disappear when scrolling with mouse ([#7](https://github.com/LuDog71FR/iced-code-editor/issues/7))

## [0.3.3] - 2026-01-22

### Fixed

- fix: 中文 will panicked ([#9](https://github.com/LuDog71FR/iced-code-editor/issues/9))

### Added

- feat: add support for Asian character input in the editor
- feat: Add support for CJK font
- feat: allow changing the font of the editor
  Default font: iced::Font::MONOSPACE

## [0.3.2] - 2026-01-16

- fix: keyboard events are interpreted when editor has no more the focus ([#6](https://github.com/LuDog71FR/iced-code-editor/issues/6))
- fix: reduce gutter for line numbers
- feat: hide/display line numbers ([#5](https://github.com/LuDog71FR/iced-code-editor/issues/5))
- feat: hide cursor if editor don't have the focus.

## [0.3.1] - 2026-01-11

### Fixed

- fix: duplicate char with two widgets on the window ([#4](https://github.com/LuDog71FR/iced-code-editor/issues/4))
- fix: panic with not english chars ([#3](https://github.com/LuDog71FR/iced-code-editor/issues/3))


## [0.3.0] - 2026-01-09

### Changed

- **BREAKING**: Removed `theme::dark()` and `theme::light()` functions
- **BREAKING**: Changed default theme to use `theme::from_iced_theme()` which auto-adapts to any Iced theme

### Added

- feat: Search and replace text

  - Dialog box to search/replace text
  - Pagination thru results
  - Replace one by one or all
  - Undo capability
  - translations file created for en, fr and es (in `locales/` folder)

- feat: line wrapping

  - Long lines are split into multiple visual lines at viewport width
  - Continuation lines display a ↪ indicator in the gutter
  - Toggle feature on/off via checkbox in editor toolbar
  - Cursor navigation and text selection work across wrapped lines

- feat!: native support for all built-in Iced themes

  - New `theme::from_iced_theme()` function that automatically adapts editor colors to any Iced theme palette
  - Color helper functions for optimal code editor appearance (darken, lighten, dim_color, with_alpha)
  - Demo app now uses native Iced theme system with full theme picker

## [0.2.9] - 2026-01-08

### Fixed

fix: prevent visual artifacts when switching to shorter content
Use the new `reset()` function instead of creating again a new code editor !
fix: prevent mouse to capture events when out of bounds

## [0.2.8] - 2026-01-08

### Fixed

fix: prevent editor background overflow when resizing panes

## [0.2.7] - 2026-01-08

### Fixed

fix: scrollable height now respects parent container bounds

## [0.2.6] - 2026-01-07

### Fixed

fix: canvas background now respects viewport height instead of content height

## [0.2.5] - 2026-01-03

### Added

- Add html, xml, css, json and md languages ([#2](https://github.com/LuDog71FR/iced-code-editor/issues/2)).

## [0.2.4] - 2025-12-27

### Fixed

- Key Space not sending to iced-code-editor ([#1](https://github.com/LuDog71FR/iced-code-editor/issues/1))

### Changed

- Better handle keyboard entries

## [0.2.3] - 2025-12-19

### Fixed

- Fix example code in README & lib

## [0.2.2] - 2025-12-19

### Fixed

- Fix GitHub repository link in Cargo.toml

## [0.2.1] - 2025-12-19

### Added

- Add build badge in README.md

### Changed

- Fix GitHub repository link in README.md

## [0.2.0] - 2025-12-19

### Added

- Initial release on crates.io
- Canvas-based high-performance code editor widget
- Syntax highlighting for multiple programming languages (Python, Lua, Rust, JavaScript, etc.)
- Line numbers with styled gutter
- Text selection via mouse drag and keyboard shortcuts
- Clipboard operations (copy, paste)
- Undo/Redo functionality with smart command grouping
- Configurable command history with size limits
- Custom scrollbars with themed styling
- Dark and light themes with customizable colors
- Comprehensive keyboard navigation support:
  - Arrow keys (with Shift for selection)
  - Home/End keys
  - Ctrl+Home/Ctrl+End
  - Page Up/Page Down
- Modified state tracking for file save indicators
- Focus management for multiple editors
- Cursor blinking animation
- Demo application with file operations

### Documentation

- Complete README with examples and usage guide
- Inline documentation for all public APIs
- Working doctests for all examples
- Keyboard shortcuts reference
