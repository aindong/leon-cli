//! Lua (spec §4): `init.lua` loaded into a plain [`Config`] ([`load`]), or kept alive as a
//! [`Runtime`] that runs its actions and function bindings; and `leon do` scripts ([`run_script`]).
//! Both reach the server through a [`Host`].

mod api;
mod exec;
pub mod keys;
pub mod rewrite;
mod runtime;
mod sandbox;
mod script;
pub mod settings;

pub use runtime::{CallCtx, Host, Runtime, LOAD_LIMIT, MAX_DEPTH, TIMED_OUT};
pub use script::{run_script, ScriptOptions, MAX_TIMERS};
pub use settings::{Settings, REPEAT_TIME};

use std::fmt;
use std::path::{Path, PathBuf};

use keys::KeySpec;
use serde_json::Value;

/// Everything a config file declared, in the order it was declared.
#[derive(Debug, Default, Clone)]
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
    /// `leon.set` calls, over the defaults.
    pub settings: Settings,
    /// Where each setting was last set, as `file:line`; a key left at its default has none.
    pub setting_sources: std::collections::BTreeMap<String, String>,
}

/// What a restored block does with the resume command a program registered for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Resume {
    /// Type it at the new prompt; Enter runs it.
    #[default]
    Ask,
    Always,
    Never,
}

impl Resume {
    pub const ALL: [Resume; 3] = [Resume::Ask, Resume::Always, Resume::Never];

    pub fn as_str(self) -> &'static str {
        match self {
            Resume::Ask => "ask",
            Resume::Always => "always",
            Resume::Never => "never",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Binding {
    pub key: KeySpec,
    pub target: Target,
    /// `file:line` of the `leon.bind` call.
    pub source: String,
    /// `{ repeats = true }` (tmux `bind -r`): after this sequence, its last key works again without
    /// the prefix for `repeat_time`.
    pub repeats: bool,
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
    /// The one chord that enters the mode for the next key (`leon attach`'s `attach` mode), as
    /// `KeySpec::sequence` writes it.
    pub prefix: Option<String>,
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

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Debug => "debug",
            Level::Info => "info",
            Level::Warning => "warning",
            Level::Error => "error",
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.at, self.level.as_str(), self.message)
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
        .or_else(|| var("XDG_CONFIG_HOME").filter(|d| d.is_absolute()).map(|d| d.join("leon/init.lua")))
        .or_else(|| var("HOME").map(|h| h.join(".config/leon/init.lua")))
}

/// Run `path` top to bottom. A bad `leon.*` call is skipped with a diagnostic; anything else
/// that raises fails the whole load.
pub fn load(path: &Path) -> Result<Config, LoadError> {
    Runtime::load(path, Box::new(NoServer)).map(|(_, cfg)| cfg)
}

/// [`load`] `src` as if it were the file at `path`: what a config edit would load to.
pub fn load_text(path: &Path, src: &str) -> Result<Config, LoadError> {
    Runtime::load_src(path, src.as_bytes(), Box::new(NoServer)).map(|(_, cfg)| cfg)
}

struct NoServer;

impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<std::time::Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}

/// The Lua error text, without mlua's "runtime error: " prefix and Lua's traceback.
fn lua_message(e: mlua::Error) -> String {
    let msg = match e {
        mlua::Error::SyntaxError { message, .. } | mlua::Error::RuntimeError(message) => message,
        mlua::Error::CallbackError { cause, .. } => return lua_message((*cause).clone()),
        e => e.to_string(),
    };
    msg.split("\nstack traceback:").next().unwrap_or_default().to_string()
}
