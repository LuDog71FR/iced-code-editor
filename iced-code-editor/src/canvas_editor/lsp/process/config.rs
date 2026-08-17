//! LSP (Language Server Protocol) configuration module.
//!
//! This module handles language server detection, configuration, and command resolution
//! for various programming languages. It maps file extensions to language servers and
//! provides functionality to resolve the correct server command based on environment
//! variables and system availability.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a language supported by an LSP server.
///
/// Contains the language identifier and the associated server key.
///
/// # Examples
///
/// ```no_run
/// use iced_code_editor::lsp_language_for_extension;
///
/// if let Some(lang) = lsp_language_for_extension("rs") {
///     assert_eq!(lang.language_id, "rust");
///     assert_eq!(lang.server_key, "rust-analyzer");
/// }
/// ```
#[derive(Clone, Copy)]
pub struct LspLanguage {
    /// Language identifier (e.g., "rust", "python", "typescript")
    pub language_id: &'static str,
    /// Key identifying the LSP server (e.g., "rust-analyzer", "pyright")
    pub server_key: &'static str,
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
///
/// Defines how to locate and run the language server.
///
/// # Examples
///
/// ```no_run
/// use iced_code_editor::lsp_server_config;
///
/// if let Some(config) = lsp_server_config("rust-analyzer") {
///     println!("key: {}", config.key);
/// }
/// ```
#[derive(Clone, Copy)]
pub struct LspServerConfig {
    /// Unique identifier for this server configuration
    pub key: &'static str,
    /// Environment variables to check for custom server paths (checked in order)
    pub env_vars: &'static [&'static str],
    /// Default command and arguments to run the server
    pub default_command: &'static [&'static str],
}

/// Resolved command to execute an LSP server.
///
/// # Examples
///
/// ```no_run
/// use iced_code_editor::{lsp_server_config, resolve_lsp_command};
///
/// if let Some(config) = lsp_server_config("gopls") {
///     if let Ok(cmd) = resolve_lsp_command(config) {
///         println!("program: {}", cmd.program);
///     }
/// }
/// ```
pub struct LspCommand {
    /// Program path or name
    pub program: String,
    /// Command-line arguments
    pub args: Vec<String>,
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
///
/// Returns `None` if the extension is not supported.
///
/// # Examples
///
/// ```
/// use iced_code_editor::lsp_language_for_extension;
///
/// let lang = lsp_language_for_extension("rs");
/// assert!(lang.is_some());
///
/// let unknown = lsp_language_for_extension("xyz");
/// assert!(unknown.is_none());
/// ```
pub fn lsp_language_for_extension(extension: &str) -> Option<LspLanguage> {
    LSP_LANGUAGE_MAPPINGS
        .iter()
        .find(|mapping| {
            mapping
                .extensions
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .map(|mapping| LspLanguage {
            language_id: mapping.language_id,
            server_key: mapping.server_key,
        })
}

/// Looks up the LSP language configuration for a file path.
///
/// Extracts the extension and delegates to [`lsp_language_for_extension`].
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use iced_code_editor::lsp_language_for_path;
///
/// let lang = lsp_language_for_path(Path::new("main.rs"));
/// assert!(lang.is_some());
///
/// let none = lsp_language_for_path(Path::new("noext"));
/// assert!(none.is_none());
/// ```
pub fn lsp_language_for_path(path: &Path) -> Option<LspLanguage> {
    let extension = path.extension()?.to_str()?;
    lsp_language_for_extension(extension)
}

/// Retrieves the server configuration for a given server key.
///
/// # Examples
///
/// ```
/// use iced_code_editor::lsp_server_config;
///
/// let cfg = lsp_server_config("rust-analyzer");
/// assert!(cfg.is_some());
///
/// let missing = lsp_server_config("unknown-server");
/// assert!(missing.is_none());
/// ```
pub fn lsp_server_config(key: &str) -> Option<&'static LspServerConfig> {
    LSP_SERVER_CONFIGS.iter().find(|config| config.key == key)
}

/// Resolves the command to execute an LSP server.
///
/// Checks environment variables first, then falls back to the default command.
/// Special handling for rust-analyzer to support rustup-installed versions.
///
/// # Errors
///
/// Returns an error string if the program cannot be located (e.g. rust-analyzer
/// or gopls are not installed and not found via their fallback discovery logic).
///
/// # Examples
///
/// ```no_run
/// use iced_code_editor::{lsp_server_config, resolve_lsp_command};
///
/// if let Some(config) = lsp_server_config("lua-language-server") {
///     match resolve_lsp_command(config) {
///         Ok(cmd) => println!("Run: {}", cmd.program),
///         Err(e) => eprintln!("Not found: {e}"),
///     }
/// }
/// ```
pub fn resolve_lsp_command(
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
/// Returns the first non-empty value found, trimmed, or None if all are
/// unset/blank.
fn resolve_program_from_envs(env_vars: &[&str]) -> Option<String> {
    resolve_program_from_envs_with(env_vars, |var| std::env::var(var).ok())
}

/// Same as [`resolve_program_from_envs`], but takes an injectable lookup so the
/// priority order can be unit-tested without touching real process environment.
///
/// The returned value is trimmed. Emptiness is judged on the trimmed value, so
/// returning it untrimmed would accept a variable holding only whitespace as a
/// "found" path — and hand a value like `" /usr/bin/gopls"` (a shell-config
/// typo, or a CI variable with a trailing newline) straight to
/// `Command::new`, which then fails with a confusing "No such file or
/// directory".
fn resolve_program_from_envs_with(
    env_vars: &[&str],
    lookup: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    for var in env_vars {
        if let Some(path) = lookup(var) {
            let path = path.trim();
            if !path.is_empty() {
                return Some(path.to_string());
            }
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
    resolve_rust_analyzer_command_with(
        |var| std::env::var(var).ok(),
        || Command::new("rust-analyzer").arg("--version").output().is_ok(),
        || {
            let output = Command::new("rustup")
                .args(["which", "rust-analyzer"])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let path =
                String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!path.is_empty()).then_some(path)
        },
    )
}

/// Same as [`resolve_rust_analyzer_command`], but takes injectable checks for the
/// environment lookup, the PATH probe, and the `rustup which` fallback so the
/// priority order can be unit-tested without spawning real subprocesses.
fn resolve_rust_analyzer_command_with(
    lookup: impl Fn(&str) -> Option<String>,
    is_on_path: impl Fn() -> bool,
    rustup_which: impl Fn() -> Option<String>,
) -> Result<String, String> {
    if let Some(path) = resolve_program_from_envs_with(
        &["RUST_ANALYZER", "RUST_ANALYZER_PATH"],
        &lookup,
    ) {
        return Ok(path);
    }
    if is_on_path() {
        return Ok("rust-analyzer".to_string());
    }
    if let Some(path) = rustup_which() {
        return Ok(path);
    }
    Err(
        "rust-analyzer not found. Please run rustup component add rust-analyzer or brew install rust-analyzer"
            .to_string(),
    )
}

/// Resolves the gopls command with special handling.
/// Checks in order:
/// 1. GOPLS / GOPLS_PATH environment variables
/// 2. Direct gopls command on PATH
/// 3. `$GOBIN/gopls`
/// 4. `$GOPATH/bin/gopls` for each `GOPATH` entry (platform path-list
///    separator: `;` on Windows, `:` elsewhere)
fn resolve_gopls_command() -> Result<String, String> {
    resolve_gopls_command_with(
        |var| std::env::var(var).ok(),
        || Command::new("gopls").arg("version").output().is_ok(),
        || go_env_var("GOBIN"),
        || go_env_var("GOPATH"),
        Path::exists,
    )
}

/// Runs `go env <var>` and returns its trimmed stdout, or `None` on failure.
fn go_env_var(var: &str) -> Option<String> {
    let output = Command::new("go").args(["env", var]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Same as [`resolve_gopls_command`], but takes injectable checks for the
/// environment lookup, the PATH probe, the `$GOBIN`/`$GOPATH` lookups, and the
/// filesystem existence check, so the priority order can be unit-tested without
/// spawning real subprocesses or touching the real filesystem.
fn resolve_gopls_command_with(
    lookup: impl Fn(&str) -> Option<String>,
    is_on_path: impl Fn() -> bool,
    gobin: impl Fn() -> Option<String>,
    gopath: impl Fn() -> Option<String>,
    exists: impl Fn(&Path) -> bool,
) -> Result<String, String> {
    if let Some(path) =
        resolve_program_from_envs_with(&["GOPLS", "GOPLS_PATH"], &lookup)
    {
        return Ok(path);
    }
    if is_on_path() {
        return Ok("gopls".to_string());
    }
    if let Some(gobin_path) = gobin() {
        let candidate = PathBuf::from(gobin_path).join("gopls");
        if exists(&candidate) {
            return Ok(candidate.to_string_lossy().to_string());
        }
    }
    if let Some(gopath_value) = gopath() {
        // Platform path-list separator (`;` on Windows, `:` elsewhere), not a
        // literal colon: `GOPATH` follows the same convention as `PATH`.
        for path in std::env::split_paths(&gopath_value) {
            if path.as_os_str().is_empty() {
                continue;
            }
            let candidate = path.join("bin").join("gopls");
            if exists(&candidate) {
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
///
/// Creates the configuration directory and an empty config file if they don't exist.
/// This prevents rust-analyzer from failing on first run on macOS.
///
/// # Examples
///
/// ```no_run
/// use iced_code_editor::ensure_rust_analyzer_config;
///
/// // Safe to call on any platform; no-op on non-macOS.
/// ensure_rust_analyzer_config();
/// ```
#[cfg(target_os = "macos")]
pub fn ensure_rust_analyzer_config() {
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
///
/// # Examples
///
/// ```no_run
/// use iced_code_editor::ensure_rust_analyzer_config;
///
/// // Safe to call on any platform; no-op on non-macOS.
/// ensure_rust_analyzer_config();
/// ```
#[cfg(not(target_os = "macos"))]
pub fn ensure_rust_analyzer_config() {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup_from(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<&str, &str> = vars.iter().copied().collect();
        move |var: &str| map.get(var).map(|v| v.to_string())
    }

    // ---- lsp_language_for_extension ----

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_lsp_language_for_extension_known() {
        let lang = lsp_language_for_extension("rs").unwrap();
        assert_eq!(lang.language_id, "rust");
        assert_eq!(lang.server_key, "rust-analyzer");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_lsp_language_for_extension_is_case_insensitive() {
        let lang = lsp_language_for_extension("RS").unwrap();
        assert_eq!(lang.language_id, "rust");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_lsp_language_for_extension_shared_by_multiple_extensions() {
        let jsx = lsp_language_for_extension("jsx").unwrap();
        let ts = lsp_language_for_extension("tsx").unwrap();
        assert_eq!(jsx.server_key, "typescript-language-server");
        assert_eq!(ts.language_id, "typescript");
    }

    #[test]
    fn test_lsp_language_for_extension_unknown() {
        assert!(lsp_language_for_extension("xyz").is_none());
    }

    // ---- lsp_language_for_path ----

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_lsp_language_for_path_known_extension() {
        let lang = lsp_language_for_path(Path::new("src/main.go")).unwrap();
        assert_eq!(lang.server_key, "gopls");
    }

    #[test]
    fn test_lsp_language_for_path_no_extension() {
        assert!(lsp_language_for_path(Path::new("Makefile")).is_none());
    }

    #[test]
    fn test_lsp_language_for_path_unknown_extension() {
        assert!(lsp_language_for_path(Path::new("data.xyz")).is_none());
    }

    // ---- lsp_server_config ----

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_lsp_server_config_known() {
        let cfg = lsp_server_config("gopls").unwrap();
        assert_eq!(cfg.env_vars, &["GOPLS", "GOPLS_PATH"]);
        assert_eq!(cfg.default_command, &["gopls"]);
    }

    #[test]
    fn test_lsp_server_config_unknown() {
        assert!(lsp_server_config("unknown-server").is_none());
    }

    // ---- resolve_program_from_envs_with ----

    #[test]
    fn test_resolve_program_from_envs_with_first_set_var_wins() {
        let lookup = lookup_from(&[("SECOND", "/bin/second")]);
        let result =
            resolve_program_from_envs_with(&["FIRST", "SECOND"], lookup);
        assert_eq!(result, Some("/bin/second".to_string()));
    }

    #[test]
    fn test_resolve_program_from_envs_with_skips_blank_values() {
        let lookup =
            lookup_from(&[("FIRST", "   "), ("SECOND", "/bin/second")]);
        let result =
            resolve_program_from_envs_with(&["FIRST", "SECOND"], lookup);
        assert_eq!(result, Some("/bin/second".to_string()));
    }

    #[test]
    fn test_resolve_program_from_envs_with_none_when_all_unset() {
        let lookup = lookup_from(&[]);
        let result =
            resolve_program_from_envs_with(&["FIRST", "SECOND"], lookup);
        assert_eq!(result, None);
    }

    #[test]
    fn test_resolve_program_from_envs_with_trims_surrounding_whitespace() {
        // Emptiness is judged on the trimmed value, so the returned value must
        // be trimmed too. A shell-config typo or a CI variable carrying a
        // trailing newline would otherwise reach `Command::new` verbatim and
        // fail with a confusing "No such file or directory".
        let lookup = lookup_from(&[("FIRST", "  /bin/first\n")]);
        let result =
            resolve_program_from_envs_with(&["FIRST", "SECOND"], lookup);
        assert_eq!(result, Some("/bin/first".to_string()));
    }

    // ---- resolve_rust_analyzer_command_with ----

    #[test]
    fn test_resolve_rust_analyzer_command_with_env_var_takes_priority() {
        let lookup = lookup_from(&[("RUST_ANALYZER", "/custom/rust-analyzer")]);
        let result =
            resolve_rust_analyzer_command_with(lookup, || true, || None);
        assert_eq!(result, Ok("/custom/rust-analyzer".to_string()));
    }

    #[test]
    fn test_resolve_rust_analyzer_command_with_falls_back_to_path() {
        let lookup = lookup_from(&[]);
        let result =
            resolve_rust_analyzer_command_with(lookup, || true, || None);
        assert_eq!(result, Ok("rust-analyzer".to_string()));
    }

    #[test]
    fn test_resolve_rust_analyzer_command_with_falls_back_to_rustup() {
        let lookup = lookup_from(&[]);
        let result = resolve_rust_analyzer_command_with(
            lookup,
            || false,
            || Some("/home/user/.rustup/bin/rust-analyzer".to_string()),
        );
        assert_eq!(
            result,
            Ok("/home/user/.rustup/bin/rust-analyzer".to_string())
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_resolve_rust_analyzer_command_with_returns_err_when_nothing_found()
    {
        let lookup = lookup_from(&[]);
        let result =
            resolve_rust_analyzer_command_with(lookup, || false, || None);
        result.unwrap_err();
    }

    // ---- resolve_gopls_command_with ----

    #[test]
    fn test_resolve_gopls_command_with_env_var_takes_priority() {
        let lookup = lookup_from(&[("GOPLS", "/custom/gopls")]);
        let result = resolve_gopls_command_with(
            lookup,
            || true,
            || None,
            || None,
            |_| true,
        );
        assert_eq!(result, Ok("/custom/gopls".to_string()));
    }

    #[test]
    fn test_resolve_gopls_command_with_falls_back_to_path() {
        let lookup = lookup_from(&[]);
        let result = resolve_gopls_command_with(
            lookup,
            || true,
            || None,
            || None,
            |_| true,
        );
        assert_eq!(result, Ok("gopls".to_string()));
    }

    #[test]
    fn test_resolve_gopls_command_with_falls_back_to_gobin_when_it_exists() {
        let lookup = lookup_from(&[]);
        let result = resolve_gopls_command_with(
            lookup,
            || false,
            || Some("/home/user/go/bin".to_string()),
            || None,
            |p| p == Path::new("/home/user/go/bin/gopls"),
        );
        assert_eq!(result, Ok("/home/user/go/bin/gopls".to_string()));
    }

    #[test]
    fn test_resolve_gopls_command_with_falls_back_to_gopath_when_it_exists() {
        let lookup = lookup_from(&[]);
        let result = resolve_gopls_command_with(
            lookup,
            || false,
            || None,
            || Some("/nonexistent:/home/user/go".to_string()),
            |p| p == Path::new("/home/user/go/bin/gopls"),
        );
        assert_eq!(result, Ok("/home/user/go/bin/gopls".to_string()));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_resolve_gopls_command_with_skips_gobin_when_gopls_does_not_exist_there()
     {
        let lookup = lookup_from(&[]);
        let result = resolve_gopls_command_with(
            lookup,
            || false,
            || Some("/home/user/go/bin".to_string()),
            || Some("/home/user/go".to_string()),
            |_| false,
        );
        result.unwrap_err();
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_resolve_gopls_command_with_returns_err_when_nothing_found() {
        let lookup = lookup_from(&[]);
        let result = resolve_gopls_command_with(
            lookup,
            || false,
            || None,
            || None,
            |_| false,
        );
        result.unwrap_err();
    }
}
