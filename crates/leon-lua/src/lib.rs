//! Lua configuration (`init.lua`, spec §4): loads a config file into a plain [`Config`].
//! The Lua state is dropped after the load; running fn bindings and actions is M5b's job.

mod api;
pub mod keys;

use std::fmt;
use std::path::{Path, PathBuf};

use keys::KeySpec;
use serde_json::Value;

/// Everything a config file declared, in the order it was declared.
#[derive(Debug, Default)]
pub struct Config {
    /// Bindings after unbinds and conflict removal. Same-key bindings stack; the newest wins.
    pub bindings: Vec<Binding>,
    pub modes: Vec<Mode>,
    pub actions: Vec<Action>,
    /// `leon.unbind(key[, action])` calls, for the consumer to apply to client defaults too.
    /// An entry with a mode blocks that key in that mode.
    pub unbinds: Vec<Unbind>,
    /// Skipped calls (errors) and conflicts (warnings).
    pub diagnostics: Vec<Diagnostic>,
    /// `leon.log` lines.
    pub logs: Vec<Diagnostic>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub key: KeySpec,
    pub target: Target,
    /// `file:line` of the `leon.bind` call.
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Action {
        name: String,
        args: Option<Value>,
    },
    /// A Lua function; only the config's Lua state can run it.
    Function,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Mode {
    pub name: String,
    pub exclusive: bool,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub name: String,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub keywords: Vec<String>,
    /// An object JSON Schema built from `args`, if given.
    pub args: Option<Value>,
    pub repeats: bool,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unbind {
    pub key: KeySpec,
    /// Only bindings to this action; `None` = all of them.
    pub action: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub level: Level,
    /// `file:line`.
    pub at: String,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.level {
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warning => "warning",
            Level::Error => "error",
        };
        write!(f, "{}: {level}: {}", self.at, self.message)
    }
}

/// The whole load failed: unreadable file, syntax error, or an error raised outside a `leon.*` call.
#[derive(Debug)]
pub struct LoadError(pub String);

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LoadError {}

/// `$LEON_CONFIG`, else `$XDG_CONFIG_HOME/leon/init.lua`, else `~/.config/leon/init.lua`.
pub fn config_path() -> Option<PathBuf> {
    let var = |k| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    var("LEON_CONFIG")
        .or_else(|| var("XDG_CONFIG_HOME").map(|d| d.join("leon/init.lua")))
        .or_else(|| var("HOME").map(|h| h.join(".config/leon/init.lua")))
}

/// Run `path` top to bottom. A bad `leon.*` call is skipped with a diagnostic; anything else
/// that raises fails the whole load.
pub fn load(path: &Path) -> Result<Config, LoadError> {
    let src = std::fs::read(path).map_err(|e| LoadError(format!("cannot read {}: {e}", path.display())))?;
    api::run(path, &src).map_err(|e| LoadError(lua_message(e)))
}

/// The Lua error text, without mlua's "runtime error: " prefix and Lua's traceback.
fn lua_message(e: mlua::Error) -> String {
    let msg = match e {
        mlua::Error::SyntaxError { message, .. } | mlua::Error::RuntimeError(message) => message,
        e => e.to_string(),
    };
    msg.split("\nstack traceback:").next().unwrap_or_default().to_string()
}
