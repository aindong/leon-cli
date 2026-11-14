//! `leon`: the command-line client, and the server when run as `leon server run`.

mod client;
mod commands;

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
    /// List, describe and call server API methods.
    #[command(subcommand)]
    Api(ApiCmd),
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
            };
            match fail {
                Fail::Api(e) => eprintln!("leon: {e}"),
                Fail::Unreachable(m) | Fail::Other(m) => eprintln!("leon: {m}"),
            }
            ExitCode::from(code)
        }
    }
}
