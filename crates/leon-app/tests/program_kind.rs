//! Which glyph a pane's program gets: the program's name (the PTY's foreground, else argv0) looked
//! up in one table, and an agent from its OSC 7501 report. The table is data: each name in one
//! kind, no agent product names (an agent says what it is through OSC 7501).

use leon_app::icons::Icon;
use leon_app::program_kind::{kind_for, kind_of, Kind, PROGRAMS};

fn argv(s: &str) -> Vec<String> {
    s.split(' ').map(String::from).collect()
}

#[test]
fn each_kind_from_a_name() {
    for (name, kind) in [
        ("zsh", Kind::Shell),
        ("bash", Kind::Shell),
        ("fish", Kind::Shell),
        ("nvim", Kind::Editor),
        ("vim", Kind::Editor),
        ("nano", Kind::Editor),
        ("nginx", Kind::Server),
        ("postgres", Kind::Server),
        ("pytest", Kind::Test),
        ("jest", Kind::Test),
        ("htop", Kind::Monitor),
        ("top", Kind::Monitor),
        ("ssh", Kind::Remote),
        ("mosh-client", Kind::Remote),
        ("make", Kind::Generic),
        ("", Kind::Generic),
    ] {
        assert_eq!(kind_of(name), kind, "{name:?}");
    }
    assert_eq!(kind_of("VIM"), Kind::Editor, "case doesn't matter");
}

#[test]
fn the_foreground_comes_first_then_argv0() {
    let zsh = argv("/bin/zsh -l");
    assert_eq!(kind_for(Some(Some("htop")), &zsh, false), Kind::Monitor, "a program run in a shell");
    assert_eq!(kind_for(Some(Some("zsh")), &zsh, false), Kind::Shell, "the shell at its prompt");
    assert_eq!(kind_for(None, &argv("/usr/bin/ssh devbox"), false), Kind::Remote, "an older server: argv0");
    assert_eq!(kind_for(None, &argv("-zsh"), false), Kind::Shell, "a login shell's argv0");
    assert_eq!(kind_for(None, &[], false), Kind::Generic);
    // null is the server naming nothing (another user's program, `sudo vim`): never argv0's shell
    assert_eq!(kind_for(Some(None), &zsh, false), Kind::Generic);
    assert_eq!(kind_for(Some(Some("node")), &zsh, false), Kind::Generic, "a name the table doesn't know");
}

#[test]
fn an_agent_is_what_reports_itself() {
    let zsh = argv("/bin/zsh");
    // an agent's process is an interpreter or its own binary: its OSC 7501 report names it
    assert_eq!(kind_for(Some(Some("node")), &zsh, true), Kind::Agent);
    assert_eq!(kind_for(Some(Some("node")), &zsh, false), Kind::Generic, "no report, no agent");
    // a record the agent left (done, idle) outlives it: a program the table knows wins
    assert_eq!(kind_for(Some(Some("zsh")), &zsh, true), Kind::Shell);
    assert_eq!(kind_for(Some(Some("vim")), &zsh, true), Kind::Editor);
    assert_eq!(kind_for(Some(Some("htop")), &zsh, true), Kind::Monitor);
}

#[test]
fn the_table_is_one_place_each_name_once() {
    let mut seen = std::collections::HashSet::new();
    for (kind, names) in PROGRAMS {
        assert_ne!(*kind, Kind::Agent, "agents come from OSC 7501, not names");
        assert_ne!(*kind, Kind::Generic, "generic is what the table doesn't name");
        for n in *names {
            assert_eq!(*n, n.to_lowercase(), "names are lower case: {n}");
            assert!(seen.insert(*n), "{n} is in two kinds");
        }
    }
}

#[test]
fn every_kind_has_its_own_glyph() {
    let glyphs: Vec<Icon> = Kind::ALL.iter().map(|k| k.icon()).collect();
    assert_eq!(Kind::ALL.len(), 8, "shell, agent, editor, server, test, monitor, remote, generic");
    let unique: std::collections::HashSet<_> = glyphs.iter().collect();
    assert_eq!(unique.len(), glyphs.len(), "{glyphs:?}");
    assert_eq!(Kind::Shell.icon(), Icon::Terminal);
    let status = [Icon::Working, Icon::Done, Icon::Blocked, Icon::Question, Icon::Permission, Icon::Auth, Icon::Error];
    assert!(glyphs.iter().all(|g| !status.contains(g)), "a program's glyph is never a status glyph");
    let names: Vec<&str> = Kind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(names, ["shell", "agent", "editor", "server", "test", "monitor", "remote", "generic"]);
}
