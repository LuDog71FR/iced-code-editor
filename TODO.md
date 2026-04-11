# TODO

## Bugs

- Ghost cursor and line highlight appears when scrolling or resizing window #8

## Documentation

None

## Features

2. **Code folding**

   - Collapse/expand blocks
   - Indentation-based or syntax-aware

3. **Minimap**

   - Overview of entire file
   - Clickable navigation

## Performance Improvements

1. **Rope data structure** for better large-file performance
2. **Incremental syntax highlighting** to avoid re-highlighting entire file
3. **Web Worker for highlighting** (when targeting WASM)
