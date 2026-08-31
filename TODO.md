# TODO

## Bugs

- Ghost cursor and line highlight appears when scrolling or resizing window #8

## Documentation

None

## Features

### Testing

None

### Editing

1. **Snippets**

### LSP / IntelliSense (completion, hover, go-to-definition, formatting already done)

2. **Diagnostics display** (underlines + gutter + panel)
3. **Find references** + **Rename symbol**
4. **Signature help**
5. **Code actions / quick fixes**
6. **Outline / document symbols** (+ breadcrumbs)

### Navigation / UI

7. **Minimap** (overview of entire file & clickable navigation)

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
