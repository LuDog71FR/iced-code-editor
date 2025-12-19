# AGENTS.md

## Build/Test Commands

- `cargo build` - Build all workspace members
- `cargo build --release` - Build optimized release
- `cargo test` - Run all tests. Never do it yourself, ask me to do it.
- `cargo test <test_name>` - Run single test (e.g., `cargo test test_me`)
- `cargo clippy` - Lint code
- `cargo fmt` - Format code

## Code Style Guidelines

- **Lints**: `unsafe_code = "forbid"`
- **Edition**: 2024, async/await with tokio runtime
- **Naming**: PascalCase for structs/enums, snake_case for functions/modules
- **Imports**: Group in order: std, external crates (sqlx, chrono, etc.), then crate/internal

## Steps to follow when modifying source code

1. **Analyze context** - Review similar methods or those in the file to be modified to use the same coding pattern
2. **Refactoring** - Perform refactoring if necessary, consolidate duplicated code when possible
3. **Compilation warnings** - Fix all compilation warnings
4. **Clippy linter** - Fix all clippy warnings
5. **Formatting** - Apply formatting with `cargo fmt`
6. **Tests** - Re-run only the tests impacted by the changes

## ⚠️ Strict rules

- **Mandatory tests**: Any new function must have a unit test (exception: frontend display management functions)
- **Mandatory documentation**: Any struct, enum or function must have a clear documentation. Every public item, with examples.
- **Test verification**: Any function modification requires verification and update of associated unit tests
- Each clippy warning is considered like an error and must be fix
- All documentation (in source code or Markdown files) must be written in English
- Never use `#[allow(dead_code)]`

## Communication Guidelines

- **Code sharing**: Do not display source code in the chat, only when requested
- **Response style**: Keep explanations concise and focused; provide detailed explanations only when requested
- **Changes summary**: Always list what was modified (files, functions), do not show entire code
- **Error context**: When reporting issues, include relevant error messages and file paths
- **Assumptions**: Explicitly state any assumptions made during implementation
- **Next steps**: Suggest logical next steps after completing a task
