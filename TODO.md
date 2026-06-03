# TODO

## Bugs

- Ghost cursor and line highlight appears when scrolling or resizing window #8

## Documentation

None

## Features

### Editing

1. **Toggle comment** (line/block, `Ctrl+/`)
2. **Auto-closing brackets/quotes** + surround selection
3. **Matching bracket highlight** + indentation guides
4. **Move / duplicate line** (`Alt+Up/Down`, `Ctrl+Shift+D`)
5. **Snippets**

### LSP / IntelliSense (completion, hover, go-to-definition already done)

6. **Diagnostics display** (underlines + gutter + panel)
7. **Find references** + **Rename symbol**
8. **Document formatting** (format on save)
9. **Signature help**
10. **Code actions / quick fixes**
11. **Outline / document symbols** (+ breadcrumbs)

### Navigation / UI

12. **Command palette** (`Ctrl+Shift+P`)
13. **Project-wide search** (find in files)
14. **Sticky scroll** (pinned scope header)
15. **Bracket pair colorization** (rainbow brackets)
16. **Whitespace / control-character rendering**
17. **Inline color preview** (swatches for `#rrggbb`)
18. **Minimap** (overview of entire file & clickable navigation)
19. **Vim mode**

## Performance Improvements

1. **Rope data structure** for better large-file performance
   - `TextBuffer` currently stores text as `Vec<String>` (one `String` per line).
     Line insert/remove shifts the `Vec` (O(n)) and in-line edits reallocate the
     whole `String`. A rope (e.g. `ropey`/`crop`) would make edits in large files cheaper.

2. **Web Worker for highlighting** (when targeting WASM)
   - Highlighting (`highlight_line_spans`) runs synchronously with syntect on the main
     thread during rendering. A per-line highlight cache (`highlighted_line_cached`)
     already amortizes the cost, but nothing is offloaded off-thread.
     A Web Worker would move highlighting off the UI thread on WASM.
