//! `leon`: the command-line client, and the server when run as `leon server run`.

mod blocks;
mod client;
mod commands;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::builder::BoolishValueParser;
use clap::{Parser, Subcommand};

use client::Fail;

#[derive(Parser)]
#[command(name = "leon", version, about = "A fast, scriptable terminal multiplexer.")]
pub struct Cli {
    /// Start a personal server when none is running.
    #[arg(long, global = true, env = "LEON_AUTOSTART", default_value = "true", num_args = 0..=1,
          require_equals = true, default_missing_value = "true", value_parser = BoolishValueParser::new())]
    pub autostart: bool,
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    pub json: bool,
    /// Target session: an ID, label, position or ID suffix [default: $LEON_SESSION].
    #[arg(short = 's', long = "session", global = true)]
    pub session: Option<String>,
    /// Target block: an ID, label, position or ID suffix [default: $LEON_BLOCK, else the active window's].
    #[arg(short = 'b', long = "block", global = true)]
    pub block: Option<String>,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Run, start, stop or check the server.
    #[command(subcommand)]
    Server(ServerCmd),
    /// Create a session and print its ID.
    New { label: Option<String> },
    /// List sessions.
    Ls,
    /// Destroy sessions.
    Kill {
        #[arg(required = true)]
        sessions: Vec<String>,
    },
    /// Manage sessions.
    #[command(subcommand)]
    Session(SessionCmd),
    /// Manage windows (tabs) in a session.
    #[command(subcommand)]
    Window(WindowCmd),
    /// Run a command in a new window and print its block ID.
    Run {
        /// Label for the new block.
        #[arg(long)]
        label: Option<String>,
        /// Keep the block open after the program exits.
        #[arg(long)]
        keep_open: bool,
        /// Wait for the program and exit with its status.
        #[arg(long)]
        wait: bool,
        /// Working directory [default: the current directory].
        #[arg(short = 'c', long = "cwd")]
        cwd: Option<PathBuf>,
        #[arg(last = true, required = true, value_name = "CMD")]
        command: Vec<String>,
    },
    /// Type TEXT (or stdin) into a terminal; no Enter is added.
    Send { text: Option<String> },
    /// Press keys in binding syntax: enter, escape, tab, ctrl+c, up, alt+left, a…
    SendKey {
        #[arg(required = true)]
        keys: Vec<String>,
    },
    /// Print a terminal's scrollback and screen.
    Capture {
        /// Drop trailing spaces and trailing blank lines.
        #[arg(long)]
        trim: bool,
    },
    /// Wait for a terminal's program to exit, and exit with its status.
    Wait {
        block: Option<String>,
        /// Give up after this long (e.g. 500ms, 10s, 2m) and exit 1.
        #[arg(long = "for", value_parser = blocks::parse_duration)]
        timeout: Option<std::time::Duration>,
    },
    /// Manage blocks (terminals).
    #[command(subcommand)]
    Block(BlockCmd),
    /// List, describe and call server API methods.
    #[command(subcommand)]
    Api(ApiCmd),
    /// Check the Lua config.
    #[command(subcommand)]
    Config(ConfigCmd),
}

#[derive(Subcommand)]
pub enum ConfigCmd {
    /// Load a config without applying it: print counts and diagnostics, exit 1 on error. Needs no server.
    Check {
        /// [default: $LEON_CONFIG, else $XDG_CONFIG_HOME/leon/init.lua, else ~/.config/leon/init.lua]
        path: Option<std::path::PathBuf>,
    },
}

#[derive(Subcommand)]
pub enum BlockCmd {
    /// List a session's blocks.
    Ls,
    /// Show a block: window, pid, command, exit status.
    Inspect { target: Option<String> },
    /// Close a block and end its program.
    Close { target: Option<String> },
}

#[derive(Subcommand)]
pub enum ServerCmd {
    /// Run the server in the foreground.
    Run {
        #[arg(long, hide = true)]
        autostarted: bool,
    },
    /// Start a server in the background (does nothing if one runs).
    Start,
    /// Stop the server.
    Stop,
    /// Show whether a server runs (exit 4 if not).
    Status,
}

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Create a session and print its ID.
    New { label: Option<String> },
    /// List sessions.
    Ls,
    /// Show a session and its windows.
    Inspect { target: Option<String> },
    /// Rename a session.
    Rename { target: String, label: String },
    /// Destroy sessions.
    Kill {
        #[arg(required = true)]
        targets: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum WindowCmd {
    /// Add a window and print its ID.
    New { label: Option<String> },
    /// List windows in tab order.
    Ls,
    /// Rename a window.
    Rename { target: String, label: String },
    /// Close a window (closing the last one destroys the session).
    Close { target: String },
}

#[derive(Subcommand)]
pub enum ApiCmd {
    /// List every method.
    List,
    /// Show a method's description and JSON Schemas.
    Describe { method: String },
    /// Call a method with JSON params (inline, @file, or - for stdin); -s fills session_id.
    Call { method: String, args: Option<String> },
}

fn main() -> ExitCode {
    let cli = Cli::parse(); // usage errors exit 2 here
    match commands::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fail) => {
            let code = match &fail {
                Fail::Api(e) if e.is_target_error() => 3,
                Fail::Api(_) | Fail::Other(_) => 1,
                Fail::Unreachable(_) => 4,
                Fail::Exit(code) => *code,
            };
            match fail {
                Fail::Api(e) => eprintln!("leon: {e}"),
                Fail::Unreachable(m) | Fail::Other(m) => eprintln!("leon: {m}"),
                Fail::Exit(_) => {}
            }
            ExitCode::from(code.clamp(0, 255) as u8)
        }
    }
}
