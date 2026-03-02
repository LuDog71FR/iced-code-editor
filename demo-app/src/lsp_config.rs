//! LSP (Language Server Protocol) configuration module.
//!
//! This module handles language server detection, configuration, and command resolution
//! for various programming languages. It maps file extensions to language servers and
//! provides functionality to resolve the correct server command based on environment
//! variables and system availability.

#![cfg(not(target_arch = "wasm32"))]

use crate::types::Template;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a language supported by an LSP server.
/// Contains the language identifier and the associated server key.
#[derive(Clone, Copy)]
pub(crate) struct LspLanguage {
    /// Language identifier (e.g., "rust", "python", "typescript")
    pub(crate) language_id: &'static str,
    /// Key identifying the LSP server (e.g., "rust-analyzer", "pyright")
    pub(crate) server_key: &'static str,
}

/// Internal mapping between file extensions and language/server configurations.
#[derive(Clone, Copy)]
struct LspLanguageMapping {
    /// File extensions associated with this language (e.g., ["rs"], ["ts", "tsx"])
    extensions: &'static [&'static str],
    /// Language identifier for LSP protocol
    language_id: &'static str,
    /// Key to look up the server configuration
    server_key: &'static str,
}

/// Configuration for an LSP server.
/// Defines how to locate and run the language server.
#[derive(Clone, Copy)]
pub(crate) struct LspServerConfig {
    /// Unique identifier for this server configuration
    pub(crate) key: &'static str,
    /// Environment variables to check for custom server paths (checked in order)
    pub(crate) env_vars: &'static [&'static str],
    /// Default command and arguments to run the server
    pub(crate) default_command: &'static [&'static str],
}

/// Resolved command to execute an LSP server.
pub(crate) struct LspCommand {
    /// Program path or name
    pub(crate) program: String,
    /// Command-line arguments
    pub(crate) args: Vec<String>,
}

/// Supported language mappings: file extensions -> language ID -> server key
const LSP_LANGUAGE_MAPPINGS: &[LspLanguageMapping] = &[
    LspLanguageMapping {
        extensions: &["rs"],
        language_id: "rust",
        server_key: "rust-analyzer",
    },
    LspLanguageMapping {
        extensions: &["py"],
        language_id: "python",
        server_key: "pyright",
    },
    LspLanguageMapping {
        extensions: &["js", "jsx"],
        language_id: "javascript",
        server_key: "typescript-language-server",
    },
    LspLanguageMapping {
        extensions: &["ts", "tsx"],
        language_id: "typescript",
        server_key: "typescript-language-server",
    },
    LspLanguageMapping {
        extensions: &["lua"],
        language_id: "lua",
        server_key: "lua-language-server",
    },
    LspLanguageMapping {
        extensions: &["go"],
        language_id: "go",
        server_key: "gopls",
    },
];

/// Server configurations for each supported LSP server.
/// Defines environment variables and default commands for each server.
const LSP_SERVER_CONFIGS: &[LspServerConfig] = &[
    LspServerConfig {
        key: "rust-analyzer",
        env_vars: &["RUST_ANALYZER", "RUST_ANALYZER_PATH"],
        default_command: &["rust-analyzer"],
    },
    LspServerConfig {
        key: "pyright",
        env_vars: &["PYRIGHT_LANGSERVER", "PYRIGHT_LANGSERVER_PATH"],
        default_command: &["pyright-langserver", "--stdio"],
    },
    LspServerConfig {
        key: "typescript-language-server",
        env_vars: &[
            "TYPESCRIPT_LANGUAGE_SERVER",
            "TYPESCRIPT_LANGUAGE_SERVER_PATH",
        ],
        default_command: &["typescript-language-server", "--stdio"],
    },
    LspServerConfig {
        key: "lua-language-server",
        env_vars: &["LUA_LANGUAGE_SERVER", "LUA_LANGUAGE_SERVER_PATH"],
        default_command: &["lua-language-server"],
    },
    LspServerConfig {
        key: "gopls",
        env_vars: &["GOPLS", "GOPLS_PATH"],
        default_command: &["gopls"],
    },
];

/// Looks up the LSP language configuration for a file extension.
/// Returns None if the extension is not supported.
pub(crate) fn lsp_language_for_extension(
    extension: &str,
) -> Option<LspLanguage> {
    let extension = extension.to_lowercase();
    LSP_LANGUAGE_MAPPINGS
        .iter()
        .find(|mapping| {
            mapping
                .extensions
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(extension.as_str()))
        })
        .map(|mapping| LspLanguage {
            language_id: mapping.language_id,
            server_key: mapping.server_key,
        })
}

/// Looks up the LSP language configuration for a file path.
/// Extracts the extension and delegates to lsp_language_for_extension.
pub(crate) fn lsp_language_for_path(path: &Path) -> Option<LspLanguage> {
    let extension = path.extension()?.to_str()?;
    lsp_language_for_extension(extension)
}

/// Looks up the LSP language configuration for a template.
/// All built-in templates use Lua syntax.
pub(crate) fn lsp_language_for_template(
    template: Template,
) -> Option<LspLanguage> {
    let extension = match template {
        Template::Empty => "lua",
        Template::HelloWorld => "lua",
        Template::Fibonacci => "lua",
        Template::Factorial => "lua",
    };
    lsp_language_for_extension(extension)
}

/// Retrieves the server configuration for a given server key.
pub(crate) fn lsp_server_config(key: &str) -> Option<&'static LspServerConfig> {
    LSP_SERVER_CONFIGS.iter().find(|config| config.key == key)
}

/// Resolves the command to execute an LSP server.
/// Checks environment variables first, then falls back to the default command.
/// Special handling for rust-analyzer to support rustup-installed versions.
pub(crate) fn resolve_lsp_command(
    config: &LspServerConfig,
) -> Result<LspCommand, String> {
    let program = if config.key == "rust-analyzer" {
        resolve_rust_analyzer_command()?
    } else if config.key == "gopls" {
        resolve_gopls_command()?
    } else {
        resolve_program_from_envs(config.env_vars)
            .unwrap_or_else(|| config.default_command[0].to_string())
    };
    let args = config
        .default_command
        .iter()
        .skip(1)
        .map(|arg| arg.to_string())
        .collect();
    Ok(LspCommand { program, args })
}

/// Resolves a program path from a list of environment variables.
/// Returns the first non-empty value found, or None if all are unset/empty.
fn resolve_program_from_envs(env_vars: &[&str]) -> Option<String> {
    for var in env_vars {
        if let Ok(path) = std::env::var(var)
            && !path.trim().is_empty()
        {
            return Some(path);
        }
    }
    None
}

/// Resolves the rust-analyzer command with special handling.
/// Checks in order:
/// 1. RUST_ANALYZER environment variable
/// 2. RUST_ANALYZER_PATH environment variable
/// 3. Direct rust-analyzer command
/// 4. rustup which rust-analyzer
fn resolve_rust_analyzer_command() -> Result<String, String> {
    if let Ok(path) = std::env::var("RUST_ANALYZER")
        && !path.trim().is_empty()
    {
        return Ok(path);
    }
    if let Ok(path) = std::env::var("RUST_ANALYZER_PATH")
        && !path.trim().is_empty()
    {
        return Ok(path);
    }
    if Command::new("rust-analyzer").arg("--version").output().is_ok() {
        return Ok("rust-analyzer".to_string());
    }
    if let Ok(output) =
        Command::new("rustup").args(["which", "rust-analyzer"]).output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }
    Err(
        "rust-analyzer not found. Please run rustup component add rust-analyzer or brew install rust-analyzer"
            .to_string(),
    )
}

fn resolve_gopls_command() -> Result<String, String> {
    if let Some(path) = resolve_program_from_envs(&["GOPLS", "GOPLS_PATH"]) {
        return Ok(path);
    }
    if Command::new("gopls").arg("version").output().is_ok() {
        return Ok("gopls".to_string());
    }
    if let Ok(output) = Command::new("go").args(["env", "GOBIN"]).output()
        && output.status.success()
    {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            let candidate = PathBuf::from(path).join("gopls");
            if candidate.exists() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }
    if let Ok(output) = Command::new("go").args(["env", "GOPATH"]).output()
        && output.status.success()
    {
        let paths = String::from_utf8_lossy(&output.stdout);
        for path in paths.trim().split(':') {
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            let candidate = PathBuf::from(path).join("bin").join("gopls");
            if candidate.exists() {
                return Ok(candidate.to_string_lossy().to_string());
            }
        }
    }
    Err(
        "gopls not found. Please set GOPLS/GOPLS_PATH or add GOPATH/bin to PATH"
            .to_string(),
    )
}

/// Ensures rust-analyzer configuration directory exists on macOS.
/// Creates the configuration directory and an empty config file if they don't exist.
/// This prevents rust-analyzer from failing on first run on macOS.
#[cfg(target_os = "macos")]
pub(crate) fn ensure_rust_analyzer_config() {
    let Some(home) = std::env::var_os("HOME") else { return };
    let mut path = std::path::PathBuf::from(home);
    path.push("Library");
    path.push("Application Support");
    path.push("rust-analyzer");
    let _ = std::fs::create_dir_all(&path);
    path.push("rust-analyzer.toml");
    if !path.exists() {
        let _ = std::fs::write(path, "");
    }
}

/// No-op on non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub(crate) fn ensure_rust_analyzer_config() {}
