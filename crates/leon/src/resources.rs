//! The subcommands for leon's resources: `session`, `window`, `block` and `api`.

use std::path::PathBuf;

use clap::Subcommand;

#[derive(Subcommand)]
pub enum BlockCmd {
    /// List a session's blocks.
    Ls,
    /// Show a block: window, pid, command, exit status.
    Inspect {
        /// The block [default: -b, else $LEON_BLOCK].
        target: Option<String>,
    },
    /// Close a block and end its program.
    Close {
        /// The block [default: -b, else $LEON_BLOCK].
        target: Option<String>,
    },
    /// Call a block method (`program_status`, or a full name) with JSON args (inline, @file, or -).
    Call {
        /// The method: a short name like `size`, or a full `dev.leon.terminal.size`.
        method: String,
        /// Arguments as a JSON object, @file, or - for stdin.
        args: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum SessionCmd {
    /// Create a session and print its ID.
    New {
        /// Name for the session.
        label: Option<String>,
        /// First terminal's directory; relative to the caller (default: server's home).
        #[arg(short = 'c', long, value_name = "DIR")]
        cwd: Option<PathBuf>,
    },
    /// List sessions.
    Ls,
    /// Show a session and its windows.
    Inspect {
        /// The session [default: -s, else $LEON_SESSION].
        target: Option<String>,
    },
    /// Rename a session.
    Rename {
        /// The session to rename.
        target: String,
        /// Its new name.
        label: String,
    },
    /// Destroy sessions.
    Kill {
        /// Sessions to destroy: IDs, labels, positions or ID suffixes.
        #[arg(required = true)]
        targets: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum WindowCmd {
    /// Add a window and print its ID.
    New {
        /// Name for the window.
        label: Option<String>,
    },
    /// List windows in tab order.
    Ls,
    /// Rename a window.
    Rename {
        /// The window: an ID, label, 1-based tab position or ID suffix.
        target: String,
        /// Its new name.
        label: String,
    },
    /// Close a window (closing the last one destroys the session).
    Close {
        /// The window: an ID, label, 1-based tab position or ID suffix.
        target: String,
    },
}

#[derive(Subcommand)]
pub enum ApiCmd {
    /// List every method with its stability (stable, experimental, internal).
    List,
    /// Show a method's stability, description, JSON Schemas and examples.
    Describe {
        /// The method, e.g. `session.view`.
        method: String,
    },
    /// Call a method with JSON params (inline, @file, or - for stdin); -s fills session_id.
    Call {
        /// The method, e.g. `session.view`.
        method: String,
        /// Params as a JSON object, @file, or - for stdin.
        args: Option<String>,
    },
}
