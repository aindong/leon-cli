//! Which glyph a pane's program gets. The program is the PTY's foreground (`foreground` in
//! `session.list_blocks`), else argv0 (a server older than that field); an agent is a block with
//! an OSC 7501 report (docs/agents.md) running a program the table doesn't know. Only names are known, never
//! arguments, so an interpreter (`node`, `python`) or a build tool is generic: guessing from
//! them would be wrong as often as right.

#[cfg(all(feature = "gui", target_os = "macos"))]
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
    (Kind::Test, &["pytest", "jest", "vitest", "mocha", "rspec", "phpunit", "ctest", "prove", "tox", "nextest"]),
    (Kind::Monitor, &["top", "htop", "btop", "atop", "iotop", "nvtop", "glances", "watch", "iftop", "nload", "bmon"]),
    (Kind::Remote, &["ssh", "mosh", "mosh-client", "telnet", "sftp", "ftp", "et"]),
];

impl Kind {
    pub const ALL: [Kind; 8] =
        [Kind::Shell, Kind::Agent, Kind::Editor, Kind::Server, Kind::Test, Kind::Monitor, Kind::Remote, Kind::Generic];

    #[cfg(all(feature = "gui", target_os = "macos"))]
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
    // longer than any program's name (a remote server's text is uncapped): not looked at
    if name.len() > 32 {
        return Kind::Generic;
    }
    let name = name.to_lowercase();
    PROGRAMS.iter().find(|(_, names)| names.contains(&name.as_str())).map_or(Kind::Generic, |(k, _)| *k)
}

/// A pane's kind. `foreground` is the block's field: None from an older server, which falls back
/// to argv0 of `command`; `Some(None)`, nothing the server may name, is Generic. A name that gives
/// no kind (an interpreter: a `#!` script reads as one, `jest` as `node`) is looked for in the
/// first word of `title`, which shells set to the running command; else it is an agent when the
/// block has an OSC 7501 report (`agent`). A program the table knows wins: a record an agent left
/// (`done`, `idle`) outlives it.
pub fn kind_for(foreground: Option<Option<&str>>, command: &[String], title: Option<&str>, agent: bool) -> Kind {
    let named = match foreground {
        None => kind_of(&crate::switcher::program_name(command)),
        Some(None) => return Kind::Generic,
        Some(Some(name)) => kind_of(name),
    };
    if named != Kind::Generic {
        return named;
    }
    let word = title.and_then(|t| t.split_whitespace().next()).map(|w| w.rsplit('/').next().unwrap_or(w));
    match word.map(kind_of) {
        Some(k) if !matches!(k, Kind::Generic | Kind::Shell) => k,
        _ if agent => Kind::Agent,
        _ => Kind::Generic,
    }
}

/// [`kind_for`] a block as the app fetched it.
pub fn kind_of_block(b: &crate::remote::Block) -> Kind {
    let foreground = b.foreground.as_ref().map(Option::as_deref);
    kind_for(foreground, &b.command, b.title.as_deref(), !b.status.is_empty())
}

/// The kind of the pane tab `w` shows (`Session::named_tile`); None for an empty tab.
pub fn tab_kind(s: &crate::remote::Session, w: &leon_core::Window) -> Option<Kind> {
    let id = s.named_tile(w)?;
    s.blocks.iter().find(|b| b.id == id).map(kind_of_block)
}
