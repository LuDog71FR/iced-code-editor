# TODO

## Bugs

None ?? ;-)

## Features

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

