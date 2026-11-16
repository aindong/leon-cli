//! M7b criterion 8 (.agent/plans/2026-11-14-m7b-ship.md): every `leon …` in the docs' sh blocks
//! parses. Each block runs in `/bin/sh` with PATH holding only a `leon` shim: the shim runs the real
//! binary with no server to reach (so commands stop after parsing, exit 4) and logs any exit but
//! 0, 1, 3 and 4: 2 is a bad command line, anything else a crash. Other programs in a block aren't found, and the cwd is a temp dir, so a
//! block can't build, install or delete anything. The README must also list the app's shortcuts.

mod help_walk;

use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// (file, the fewest `leon` calls its sh blocks must make)
const DOCS: [(&str, usize); 9] = [
    ("README.md", 8),
    ("docs/agents.md", 5),
    ("docs/lua.md", 2),
    ("docs/cli.md", 5),
    ("docs/cli/layout.md", 5),
    ("docs/cli/sessions.md", 3),
    ("docs/cli/settings.md", 5),
    ("docs/integrators.md", 3),
    ("docs/updates.md", 3),
];

// `leon` and `leon app` open the GUI, `server run|start` would serve: skip them. A failed call
// prints a stand-in ID so `id=$(leon run …)` gives later lines something to pass.
const SHIM: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> "$DOC_CALLS"
case "$1" in ''|app) exit 0 ;; server) case "$2" in run|start) exit 0 ;; esac ;; esac
"$DOC_LEON" "$@" < /dev/null > /dev/null 2> "$DOC_ERR"
rc=$?
case $rc in 0|1|3|4) ;; *) { printf 'leon %s (exit %s)\n' "$*" $rc; while IFS= read -r l; do printf '  %s\n' "$l"; done < "$DOC_ERR"; } >> "$DOC_BAD" ;; esac
echo block:0000
exit 0
"#;

#[test]
fn doc_shell_lines_parse() {
    let tmp = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let bin = tmp.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    std::fs::write(bin.join("leon"), SHIM).unwrap();
    std::fs::set_permissions(bin.join("leon"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let (calls, bad) = (tmp.path().join("calls"), tmp.path().join("bad"));
    let mut problems = vec![];
    for (name, fewest) in DOCS {
        let doc = std::fs::read_to_string(help_walk::repo().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let _ = std::fs::remove_file(&calls);
        for block in help_walk::blocks(&doc, "sh") {
            let mut child = Command::new("/bin/sh")
                .args(["-c", &block])
                .current_dir(tmp.path())
                .env_clear()
                .env("PATH", &bin)
                .env("HOME", tmp.path())
                .env("LEON_RUNTIME_DIR", tmp.path().join("run"))
                .env("LEON_AUTOSTART", "0")
                .env("LEON_CONFIG", tmp.path().join("none.lua"))
                // a documented `leon self-update` asks a closed loopback port, never the release
                // server: it must not reach the network, let alone replace the binary under test
                .env("LEON_RELEASE_URL", "http://127.0.0.1:9")
                .env("DOC_LEON", env!("CARGO_BIN_EXE_leon"))
                .env("DOC_CALLS", &calls)
                .env("DOC_BAD", &bad)
                .env("DOC_ERR", tmp.path().join("err"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let end = Instant::now() + Duration::from_secs(20);
            while child.try_wait().unwrap().is_none() {
                if Instant::now() > end {
                    let _ = child.kill();
                    panic!("{name}: a sh block ran over 20 s:\n{block}");
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let n = std::fs::read_to_string(&calls).map_or(0, |c| c.lines().count());
        if n < fewest {
            problems.push(format!("{name}: its sh blocks call leon {n} times, expected at least {fewest}"));
        }
        if let Ok(b) = std::fs::read_to_string(&bad) {
            problems.push(format!("{name}: bad command lines:\n{b}"));
            std::fs::remove_file(&bad).unwrap();
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// A binding's "cmd+shift+d" as the README writes it: ⌘⇧D.
fn glyphs(binding: &str) -> String {
    binding
        .split('+')
        .map(|w| match w {
            "cmd" => "⌘".to_string(),
            "shift" => "⇧".to_string(),
            "alt" => "⌥".to_string(),
            "ctrl" => "⌃".to_string(),
            "left" => "←".to_string(),
            "right" => "→".to_string(),
            "up" => "↑".to_string(),
            "down" => "↓".to_string(),
            "enter" => "↩".to_string(),
            "pageup" => "PageUp".to_string(),
            "pagedown" => "PageDown".to_string(),
            k => k.to_uppercase(),
        })
        .collect()
}

#[test]
fn readme_lists_the_app_shortcuts() {
    let readme = std::fs::read_to_string(help_walk::repo().join("README.md")).unwrap();
    // the app's built-in shortcuts are the `DEFAULTS` table, one `("key", "action")` per line
    let keymap = std::fs::read_to_string(help_walk::repo().join("crates/leon-app/src/keymap.rs")).unwrap();
    let table = keymap.split("pub const DEFAULTS").nth(1).and_then(|t| t.split("];").next()).unwrap();
    let bindings: Vec<&str> = table.split("(\"").skip(1).filter_map(|s| s.split('"').next()).collect();
    assert!(bindings.len() >= 15, "found the app's bindings: {bindings:?}");
    for b in bindings {
        assert!(readme.contains(&glyphs(b)), "README should list the shortcut {b} as {}", glyphs(b));
    }
    assert!(readme.contains("⌘1"), "README should list ⌘1–⌘9");
    assert!(readme.contains("⌘Q"), "README should list ⌘Q");
}

#[test]
fn readme_and_agents_guide_cover_their_topics() {
    let readme = std::fs::read_to_string(help_walk::repo().join("README.md")).unwrap();
    let intro = readme.split("\n## ").next().unwrap();
    assert!(intro.contains("terminal multiplexer") && intro.lines().count() > 5, "README opens by saying what leon is");
    let shots: Vec<&str> = intro.split("](docs/images/").skip(1).filter_map(|s| s.split(')').next()).collect();
    assert!((2..=3).contains(&shots.len()), "README shows 2–3 screenshots or recordings up top: {shots:?}");
    for s in shots {
        let img = std::fs::read(help_walk::repo().join("docs/images").join(s)).unwrap_or_default();
        assert!(
            img.starts_with(b"\x89PNG") || img.starts_with(b"GIF89a"),
            "docs/images/{s} is a PNG or GIF in the repo"
        );
    }
    for heading in
        ["## Install", "## The first five commands", "## Split it", "## Let an agent drive it", "## App shortcuts"]
    {
        assert!(readme.lines().any(|l| l == heading), "README should have the section {heading:?}");
    }
    for topic in ["scripts/install.sh", "leon skill setup"] {
        assert!(readme.contains(topic), "README should cover {topic:?}");
    }
    let agents = std::fs::read_to_string(help_walk::repo().join("docs/agents.md")).expect("docs/agents.md exists");
    for topic in [
        "leon skill setup",
        "leon skill status",
        "leon skill remove",
        "leon skill show",
        "LEON_SESSION",
        "LEON_BLOCK",
        "set_resume",
    ] {
        assert!(agents.contains(topic), "docs/agents.md should cover {topic:?}");
    }
}
