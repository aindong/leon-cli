//! Daily-driver F4 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 1): `leon skill setup` in a
//! HOME with no ~/.claude or ~/.codex still sets up an agent whose program is on PATH, and says for
//! each agent why it was picked and what it wrote; with none found it says so. Everything stays in
//! a temp dir: HOME is there, CLAUDE_CONFIG_DIR and CODEX_HOME are unset, and PATH is pinned.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn setup(home: &Path, path: &str, rt: &Path) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["skill", "setup"])
        .env("HOME", home)
        .env("PATH", path)
        .env("LEON_RUNTIME_DIR", rt)
        .env("LEON_STATE_DIR", rt.join("state"))
        .env("LEON_AUTOSTART", "0")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .env_remove("LEON_SESSION")
        .env_remove("LEON_BLOCK")
        .output()
        .unwrap();
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

fn dirs() -> (tempfile::TempDir, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("home")).unwrap();
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    (root, rt)
}

#[test]
fn agents_on_path_get_the_skill_and_the_hook() {
    let (root, rt) = dirs();
    let home = root.path().join("home");
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    for name in ["claude", "codex"] {
        std::fs::write(bin.join(name), "#!/bin/sh\nexit 0\n").unwrap();
        std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (code, out) = setup(&home, &format!("{}:/usr/bin:/bin", bin.display()), rt.path());
    assert_eq!(code, 0, "{out}");
    for p in
        [".claude/skills/leon/SKILL.md", ".claude/settings.json", ".agents/skills/leon/SKILL.md", ".codex/hooks.json"]
    {
        assert!(home.join(p).exists(), "{p} should be written:\n{out}");
    }
    assert!(out.contains("found claude on PATH"), "says why Claude Code was set up:\n{out}");
    assert!(out.contains("found codex on PATH"), "says why Codex was set up:\n{out}");
    for p in [".claude/settings.json", ".codex/hooks.json"] {
        assert!(out.contains(&home.join(p).display().to_string()), "names {p}:\n{out}");
    }
}

#[test]
fn with_no_agent_found_it_says_so() {
    let (root, rt) = dirs();
    let home = root.path().join("home");
    let empty = root.path().join("empty-bin");
    std::fs::create_dir(&empty).unwrap();
    let (code, out) = setup(&home, &format!("{}:/usr/bin:/bin", empty.display()), rt.path());
    assert_eq!(code, 0, "{out}");
    assert!(home.join(".agents/skills/leon/SKILL.md").exists(), "{out}");
    assert!(!home.join(".claude").exists() && !home.join(".codex").exists(), "{out}");
    assert!(out.contains("no Claude Code or Codex found"), "says no agent was found:\n{out}");
    assert!(out.contains("--agent"), "says how to add one:\n{out}");
}
