# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

None

## 0.2.5 - 2026-01-03

### Fixes

None

### Changes

- Add html, xml, css, json and md languages (#2). 

## 0.2.4 - 2025-12-27

### Fixes

- Key Space not sending to iced-code-editor ([#1](https://github.com/LuDog71FR/iced-code-editor/issues/1)) 

### Changes

- Better handle keyboard entries

## [0.2.3] - 2025-12-19

### Fixes

- Fix example code in README & lib

## [0.2.2] - 2025-12-19

### Fixes

- Fix GitHub repository link in Cargo.toml

## [0.2.1] - 2025-12-19

### Added

- Add build badge in README.md

### Changes

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
