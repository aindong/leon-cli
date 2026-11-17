//! Which glyph a pane's program gets. The program is the PTY's foreground (`foreground` in
//! `session.list_blocks`), else argv0 (a server older than that field); an agent is a block with
//! an OSC 7501 report (docs/agents.md) running a program the table doesn't know. Only names are known, never
//! arguments, so an interpreter (`node`, `python`) or a build tool is generic: guessing from
//! them would be wrong as often as right.

use crate::icons::Icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Shell,
    Agent,
    Editor,
    Server,
    Test,
    Monitor,
    Remote,
    Generic,
}

/// Program names by kind, lower case. Agents aren't here: they report themselves.
pub const PROGRAMS: &[(Kind, &[&str])] = &[
    (
        Kind::Shell,
        &["sh", "bash", "zsh", "fish", "dash", "ksh", "mksh", "tcsh", "csh", "nu", "elvish", "xonsh", "pwsh", "login"],
    ),
    (
        Kind::Editor,
        &["vi", "vim", "nvim", "view", "nano", "pico", "emacs", "hx", "helix", "micro", "kak", "ed", "joe", "mg"],
    ),
    (
        Kind::Server,
        &[
            "nginx",
            "httpd",
            "apache2",
            "caddy",
            "postgres",
            "mysqld",
            "mariadbd",
            "redis-server",
            "mongod",
            "memcached",
        ],
    ),
    (
        Kind::Test,
        &["pytest", "jest", "vitest", "mocha", "rspec", "phpunit", "ctest", "bats", "prove", "tox", "nextest"],
    ),
    (Kind::Monitor, &["top", "htop", "btop", "atop", "iotop", "nvtop", "glances", "watch", "iftop", "nload", "bmon"]),
    (Kind::Remote, &["ssh", "mosh", "mosh-client", "telnet", "sftp", "ftp", "et"]),
];

impl Kind {
    pub const ALL: [Kind; 8] =
        [Kind::Shell, Kind::Agent, Kind::Editor, Kind::Server, Kind::Test, Kind::Monitor, Kind::Remote, Kind::Generic];

    pub fn icon(self) -> Icon {
        match self {
            Kind::Shell => Icon::Terminal,
            Kind::Agent => Icon::ProgramAgent,
            Kind::Editor => Icon::ProgramEditor,
            Kind::Server => Icon::ProgramServer,
            Kind::Test => Icon::ProgramTest,
            Kind::Monitor => Icon::ProgramMonitor,
            Kind::Remote => Icon::ProgramRemote,
            Kind::Generic => Icon::ProgramGeneric,
        }
    }

    /// Its name in debug selectors (`tab-program-0-editor`).
    pub fn name(self) -> &'static str {
        match self {
            Kind::Shell => "shell",
            Kind::Agent => "agent",
            Kind::Editor => "editor",
            Kind::Server => "server",
            Kind::Test => "test",
            Kind::Monitor => "monitor",
            Kind::Remote => "remote",
            Kind::Generic => "generic",
        }
    }
}

/// The kind the table gives `name`, any case; Generic for a name it doesn't have.
pub fn kind_of(name: &str) -> Kind {
    let name = name.to_lowercase();
    PROGRAMS.iter().find(|(_, names)| names.contains(&name.as_str())).map_or(Kind::Generic, |(k, _)| *k)
}

/// A pane's kind. `foreground` is the block's field: None from an older server, which falls back
/// to argv0 of `command`; `Some(None)`, nothing the server may name, is Generic. A name the table
/// doesn't know is an agent when the block has an OSC 7501 report (`agent`): an agent runs as an
/// interpreter or its own binary, and a record it left (`done`, `idle`) outlives it, so a
/// program the table knows wins.
pub fn kind_for(foreground: Option<Option<&str>>, command: &[String], agent: bool) -> Kind {
    let named = match foreground {
        None => kind_of(&crate::switcher::program_name(command)),
        Some(name) => name.map_or(Kind::Generic, kind_of),
    };
    if agent && named == Kind::Generic {
        return Kind::Agent;
    }
    named
}

/// [`kind_for`] a block as the app fetched it.
pub fn kind_of_block(b: &crate::remote::Block) -> Kind {
    kind_for(b.foreground.as_ref().map(Option::as_deref), &b.command, !b.status.is_empty())
}

/// The kind of the pane tab `w` shows (`Session::named_tile`); None for an empty tab.
pub fn tab_kind(s: &crate::remote::Session, w: &leon_core::Window) -> Option<Kind> {
    let id = s.named_tile(w)?;
    s.blocks.iter().find(|b| b.id == id).map(kind_of_block)
}
