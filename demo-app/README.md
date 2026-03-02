## Web version

The web version can be run with [`trunk`]:

```
cargo install trunk
rustup target add wasm32-unknown-unknown
trunk serve
```

## LSP Server Installation

The LSP features in the Demo App require locally installed language servers. Supported languages and servers:

- Rust: rust-analyzer
- Python: pyright-langserver
- JavaScript/TypeScript: typescript-language-server
- Lua: lua-language-server
- Go: gopls

### Installation Examples

```bash
# Rust
rustup component add rust-analyzer

# Python
npm i -g pyright

# JavaScript/TypeScript
npm i -g typescript typescript-language-server

# Lua (macOS)
brew install lua-language-server

# Go
go install golang.org/x/tools/gopls@latest
export PATH=$PATH:$(go env GOPATH)/bin
```

### Custom Paths

If a language server is not in PATH, you can specify its location via environment variables:

- RUST_ANALYZER / RUST_ANALYZER_PATH
- PYRIGHT_LANGSERVER / PYRIGHT_LANGSERVER_PATH
- TYPESCRIPT_LANGUAGE_SERVER / TYPESCRIPT_LANGUAGE_SERVER_PATH
- LUA_LANGUAGE_SERVER / LUA_LANGUAGE_SERVER_PATH
- GOPLS / GOPLS_PATH
