//! M7a (.agent/plans/2026-11-14-m7a-agent-skill.md): `leon skill show|setup|status|remove|hook`, black
//! box. Every run gets HOME, CLAUDE_CONFIG_DIR, CODEX_HOME and LEON_RUNTIME_DIR inside one temp dir, so
//! the real ~/.claude, ~/.codex and ~/.agents are never read or written.

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{json, Value};

struct Env {
    root: tempfile::TempDir,
    rt: tempfile::TempDir,
}

struct Out {
    code: i32,
    out: String,
    err: String,
}

impl Env {
    fn new() -> Env {
        let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("home")).unwrap();
        Env { root, rt }
    }
    fn home(&self) -> PathBuf {
        self.root.path().join("home")
    }
    fn claude(&self) -> PathBuf {
        self.root.path().join("claude")
    }
    fn codex(&self) -> PathBuf {
        self.root.path().join("codex")
    }
    fn claude_skill(&self) -> PathBuf {
        self.claude().join("skills/leon/SKILL.md")
    }
    fn settings(&self) -> PathBuf {
        self.claude().join("settings.json")
    }
    fn codex_skill(&self) -> PathBuf {
        self.home().join(".agents/skills/leon/SKILL.md")
    }
    fn hooks_json(&self) -> PathBuf {
        self.codex().join("hooks.json")
    }

    fn leon_env(&self, args: &[&str], env: &[(&str, &str)]) -> Out {
        for p in [self.home(), self.claude(), self.codex()] {
            assert!(p.starts_with(self.root.path()), "test paths must stay in the temp dir");
        }
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.args(args)
            .env("HOME", self.home())
            .env("CLAUDE_CONFIG_DIR", self.claude())
            .env("CODEX_HOME", self.codex())
            .env("LEON_RUNTIME_DIR", self.rt.path())
            .env("LEON_STATE_DIR", std::path::Path::new(self.rt.path()).join("state"))
            .env("LEON_AUTOSTART", "0")
            // no claude or codex on PATH: setup must not find this machine's
            .env("PATH", "/usr/bin:/bin")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("XDG_RUNTIME_DIR");
        for (k, v) in env {
            c.env(k, v);
        }
        let o = c.output().unwrap();
        assert_eq!(std::fs::read_dir(self.rt.path()).unwrap().count(), 0, "skill commands must not start a server");
        Out {
            code: o.status.code().unwrap(),
            out: String::from_utf8_lossy(&o.stdout).into_owned(),
            err: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }
    fn leon(&self, args: &[&str]) -> Out {
        self.leon_env(args, &[])
    }
    fn ok(&self, args: &[&str]) -> Out {
        let o = self.leon(args);
        assert_eq!(o.code, 0, "leon {args:?}\nstdout: {}\nstderr: {}", o.out, o.err);
        o
    }

    /// Every path under the temp root with its bytes (or link target), to prove nothing changed.
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn walk(dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                let meta = std::fs::symlink_metadata(&p).unwrap();
                if meta.file_type().is_symlink() {
                    out.insert(p.clone(), std::fs::read_link(&p).unwrap().into_os_string().into_encoded_bytes());
                } else if meta.is_dir() {
                    out.insert(p.clone(), b"<dir>".to_vec());
                    walk(&p, out);
                } else {
                    out.insert(p.clone(), std::fs::read(&p).unwrap());
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(self.root.path(), &mut out);
        out
    }

    fn status(&self) -> Value {
        serde_json::from_str(&self.ok(&["skill", "status", "--json"]).out).unwrap()
    }
    fn state(&self, agent: &str) -> String {
        let s = self.status();
        let a = s.as_array().unwrap().iter().find(|a| a["agent"] == agent).unwrap_or_else(|| panic!("{s}"));
        a["status"].as_str().unwrap().to_string()
    }
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn is_ours(hook: &Value) -> bool {
    hook["command"].as_str().is_some_and(|c| c.contains("leon skill hook"))
}

/// Our entries, across every event, and the hook file with them taken out (with the groups and
/// event lists that held only ours).
fn split_ours(v: &Value) -> (Vec<Value>, Value) {
    let mut v = v.clone();
    let mut ours = vec![];
    if let Some(events) = v["hooks"].as_object_mut() {
        for groups in events.values_mut().filter_map(Value::as_array_mut) {
            let had = groups.len();
            for g in groups.iter_mut() {
                if let Some(hooks) = g["hooks"].as_array_mut() {
                    ours.extend(hooks.iter().filter(|h| is_ours(h)).cloned());
                    hooks.retain(|h| !is_ours(h));
                }
            }
            groups.retain(|g| g["hooks"].as_array().is_none_or(|h| !h.is_empty()));
            if had > 0 && groups.is_empty() {
                *groups = vec![Value::Null]; // marked: only ours were here
            }
        }
        events.retain(|_, g| g != &json!([null]));
    }
    (ours, v)
}

/// leon's hooks per agent: SessionStart and the status events (crates/leon/src/skill/hooks.rs).
const CLAUDE_HOOKS: usize = 7;
const CODEX_HOOKS: usize = 6;

const OTHER_SETTINGS: &str = r#"{
  "model": "opus",
  "permissions": { "allow": ["Bash(ls:*)"] },
  "hooks": {
    "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "guard.sh" }] }],
    "SessionStart": [{ "matcher": "startup", "hooks": [{ "type": "command", "command": "echo hi" }] }],
    "Stop": [{ "hooks": [{ "type": "command", "command": "verify.sh", "timeout": 60 }] }]
  },
  "env": { "FOO": "1" }
}
"#;

fn keys(v: &Value) -> Vec<String> {
    v.as_object().unwrap().keys().cloned().collect()
}

// 1
#[test]
fn show_prints_the_guide_and_touches_nothing() {
    let e = Env::new();
    let before = e.snapshot();
    let g = e.ok(&["skill", "show"]).out;
    assert!(g.starts_with("---\n"), "{g}");
    assert!(g.lines().take(5).any(|l| l == "name: leon"), "{g}");
    for needle in [
        "LEON_SESSION",
        "LEON_BLOCK",
        "leon run",
        "leon send ",
        "leon send-key",
        "leon capture",
        "leon wait",
        "leon api list",
        "leon api describe",
        "leon api call",
        "--label",
        "leon block close",
    ] {
        assert!(g.contains(needle), "guide lacks {needle:?}");
    }
    for code in ["0", "1", "2", "3", "4"] {
        assert!(g.contains(&format!("`{code}`")), "guide lacks exit code {code}");
    }
    assert_eq!(e.snapshot(), before, "show wrote files");
}

// 2
#[test]
fn setup_into_an_empty_home() {
    let e = Env::new();
    e.ok(&["skill", "setup", "--agent", "claude"]);
    let skill = std::fs::read_to_string(e.claude_skill()).unwrap();
    assert!(skill.contains("<!-- leon-skill: installed by leon skill setup -->"), "{skill}");
    assert!(skill.contains("leon skill show"), "{skill}");
    assert!(skill.lines().any(|l| l == "name: leon"), "{skill}");
    let (ours, rest) = split_ours(&read_json(&e.settings()));
    assert_eq!(ours.len(), CLAUDE_HOOKS, "one leon hook per event: {ours:?}");
    assert_eq!(rest, json!({ "hooks": {} }), "{rest}");
    let group = &read_json(&e.settings())["hooks"]["SessionStart"][0];
    assert!(group.get("matcher").is_none(), "our hook runs on every session start: {group}");
    assert_eq!(group["hooks"][0]["type"], "command");
}

// 3
#[test]
fn setup_keeps_every_other_key_and_hook() {
    let e = Env::new();
    write(&e.settings(), OTHER_SETTINGS);
    let original: Value = serde_json::from_str(OTHER_SETTINGS).unwrap();
    e.ok(&["skill", "setup", "--agent", "claude"]);
    let now = read_json(&e.settings());
    let (ours, rest) = split_ours(&now);
    assert_eq!(ours.len(), CLAUDE_HOOKS, "{now}");
    assert_eq!(rest, original, "other settings changed");
    assert_eq!(keys(&now), keys(&original), "key order changed");
    let events = keys(&original["hooks"]);
    assert_eq!(keys(&now["hooks"])[..events.len()], events, "hook event order changed");
    let backup = std::fs::read_to_string(e.claude().join("settings.json.leon-backup")).unwrap();
    assert_eq!(backup, OTHER_SETTINGS, "backup must hold the original bytes");
}

// 4
#[test]
fn setup_twice_changes_nothing() {
    let e = Env::new();
    write(&e.settings(), OTHER_SETTINGS);
    e.ok(&["skill", "setup", "--agent", "claude"]);
    let after_first = e.snapshot();
    let again: Value = serde_json::from_str(&e.ok(&["skill", "setup", "--agent", "claude", "--json"]).out).unwrap();
    assert_eq!(e.snapshot(), after_first, "second setup changed files");
    let files = again[0]["files"].as_array().unwrap();
    assert!(!files.is_empty(), "{again}");
    assert!(files.iter().all(|f| f["action"] == "unchanged"), "{again}");
    assert_eq!(again[0]["status"], "current", "{again}");
}

// 5
#[test]
fn status_before_and_after() {
    let e = Env::new();
    assert_eq!(e.state("claude"), "missing");
    assert_eq!(e.state("codex"), "missing");
    e.ok(&["skill", "setup", "--agent", "claude"]);
    assert_eq!(e.state("claude"), "current");
    assert_eq!(e.state("codex"), "missing");
    let text = std::fs::read_to_string(e.claude_skill()).unwrap();
    std::fs::write(e.claude_skill(), text.replace("leon skill show", "leon skill show --old")).unwrap();
    assert_eq!(e.state("claude"), "outdated");
    e.ok(&["skill", "setup", "--agent", "claude"]);
    assert_eq!(e.state("claude"), "current", "setup brings an outdated skill up to date");
    for bad in ["{ not json", r#"{"hooks":[]}"#, r#"{"hooks":{"SessionStart":{}}}"#] {
        std::fs::write(e.settings(), bad).unwrap();
        assert_eq!(e.state("claude"), "unreadable", "{bad}");
        let before = e.snapshot();
        for cmd in [&["skill", "setup", "--agent", "claude"][..], &["skill", "remove"][..]] {
            let o = e.leon(cmd);
            assert_eq!(o.code, 1, "{cmd:?} on {bad}: {}{}", o.out, o.err);
            assert_eq!(e.snapshot(), before, "{cmd:?} touched files with an unreadable {bad}");
        }
    }
}

// 6
#[test]
fn remove_leaves_the_other_hooks() {
    let e = Env::new();
    write(&e.settings(), OTHER_SETTINGS);
    e.ok(&["skill", "setup", "--agent", "claude"]);
    e.ok(&["skill", "remove"]);
    assert!(!e.claude_skill().exists());
    assert!(!e.claude().join("skills/leon").exists(), "the empty leon dir is removed too");
    let original: Value = serde_json::from_str(OTHER_SETTINGS).unwrap();
    assert_eq!(read_json(&e.settings()), original);
    assert_eq!(e.state("claude"), "missing");
}

// 7
#[test]
fn dry_run_writes_nothing() {
    for with_settings in [false, true] {
        let e = Env::new();
        if with_settings {
            write(&e.settings(), OTHER_SETTINGS);
        }
        let before = e.snapshot();
        let o = e.ok(&["skill", "setup", "--agent", "claude", "--dry-run"]);
        assert_eq!(e.snapshot(), before, "dry run wrote files");
        assert!(o.out.contains(e.claude_skill().to_str().unwrap()), "{}", o.out);
        assert!(o.out.contains(e.settings().to_str().unwrap()), "{}", o.out);
        let o = e.ok(&["skill", "setup", "--agent", "codex", "--dry-run"]);
        assert_eq!(e.snapshot(), before, "codex dry run wrote files");
        assert!(o.out.contains(e.hooks_json().to_str().unwrap()), "{}", o.out);
    }
    let e = Env::new();
    write(&e.settings(), OTHER_SETTINGS);
    e.ok(&["skill", "setup", "--agent", "claude"]);
    let before = e.snapshot();
    e.ok(&["skill", "remove", "--dry-run"]);
    assert_eq!(e.snapshot(), before, "remove --dry-run wrote files");
}

// 8
#[test]
fn a_foreign_skill_is_reported_and_kept() {
    let e = Env::new();
    let foreign = "---\nname: leon\ndescription: someone else's\n---\nmine\n";
    write(&e.claude_skill(), foreign);
    assert_eq!(e.state("claude"), "foreign");
    let before = e.snapshot();
    let o = e.leon(&["skill", "setup", "--agent", "claude"]);
    assert_eq!(o.code, 1, "{}{}", o.out, o.err);
    assert!((o.out.clone() + &o.err).contains("--force"), "say how to override: {}{}", o.out, o.err);
    assert_eq!(e.snapshot(), before, "a refused setup must change nothing");
    e.ok(&["skill", "remove"]);
    assert_eq!(std::fs::read_to_string(e.claude_skill()).unwrap(), foreign, "remove deleted a foreign skill");
    e.ok(&["skill", "setup", "--agent", "claude", "--force"]);
    assert_eq!(e.state("claude"), "current");
    assert_eq!(std::fs::read_to_string(e.claude().join("skills/leon/SKILL.md.leon-backup")).unwrap(), foreign);
}

// 9
#[test]
fn codex_setup_and_remove() {
    let e = Env::new();
    e.ok(&["skill", "setup", "--agent", "codex"]);
    assert!(std::fs::read_to_string(e.codex_skill()).unwrap().contains("leon skill show"));
    assert_eq!(split_ours(&read_json(&e.hooks_json())).0.len(), CODEX_HOOKS);
    assert_eq!(e.state("codex"), "current");
    assert!(!e.claude().exists(), "codex setup touched claude");

    let e = Env::new();
    let other = r#"{"description":"mine","hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"x.sh"}]}],"Stop":[{"hooks":[{"type":"command","command":"y.sh"}]}]}}"#;
    write(&e.hooks_json(), other);
    let original: Value = serde_json::from_str(other).unwrap();
    e.ok(&["skill", "setup", "--agent", "codex"]);
    let (ours, rest) = split_ours(&read_json(&e.hooks_json()));
    assert_eq!(ours.len(), CODEX_HOOKS);
    assert_eq!(rest, original);
    e.ok(&["skill", "remove", "--agent", "codex"]);
    assert_eq!(read_json(&e.hooks_json()), original);
    assert!(!e.codex_skill().exists());
    assert_eq!(e.state("codex"), "missing");
}

// 10
#[test]
fn hook_names_the_session_and_block() {
    let e = Env::new();
    let o = e.leon_env(&["skill", "hook"], &[("LEON_SESSION", "session:s-1"), ("LEON_BLOCK", "block:b-2")]);
    assert_eq!(o.code, 0, "{}", o.err);
    assert!(o.out.contains("session:s-1") && o.out.contains("block:b-2"), "{}", o.out);
    assert!(o.out.contains("leon skill show"), "{}", o.out);
    let o = e.ok(&["skill", "hook"]);
    assert_eq!(o.out, "", "outside leon the hook says nothing");
}

// 11
#[test]
fn setup_without_agent_picks_installed_agents() {
    let e = Env::new();
    std::fs::create_dir(e.claude()).unwrap();
    e.ok(&["skill", "setup"]);
    assert!(e.claude_skill().exists() && e.settings().exists());
    assert!(!e.codex().exists() && !e.codex_skill().exists(), "codex isn't installed here");

    let e = Env::new();
    e.ok(&["skill", "setup"]);
    assert!(e.codex_skill().exists(), "no agent found: the skill goes to ~/.agents/skills");
    assert!(!e.claude().exists() && !e.codex().exists(), "no hook files without an agent");
    assert_eq!(e.state("codex"), "current");
}

// 12
#[test]
fn a_symlinked_settings_file_stays_a_symlink() {
    let e = Env::new();
    let target = e.root.path().join("dotfiles/claude-settings.json");
    write(&target, OTHER_SETTINGS);
    std::fs::create_dir_all(e.claude()).unwrap();
    std::os::unix::fs::symlink(&target, e.settings()).unwrap();
    e.ok(&["skill", "setup", "--agent", "claude"]);
    assert!(std::fs::symlink_metadata(e.settings()).unwrap().file_type().is_symlink());
    assert_eq!(split_ours(&read_json(&target)).0.len(), CLAUDE_HOOKS);
    e.ok(&["skill", "remove"]);
    assert!(std::fs::symlink_metadata(e.settings()).unwrap().file_type().is_symlink());
    assert_eq!(read_json(&target), serde_json::from_str::<Value>(OTHER_SETTINGS).unwrap());
}
