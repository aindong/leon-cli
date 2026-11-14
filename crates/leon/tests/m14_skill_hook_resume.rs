//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 7): the
//! SessionStart hook records the agent's own resume command for its block, refuses odd session ids,
//! records nothing for an agent with no terminal (a nested `claude -p`), does nothing outside leon,
//! never hangs, and `skill setup` upgrades the old hook command.
//!
//! The recording runs the hook inside a leon block, in the foreground of its terminal, the way an
//! interactive agent's hook runs.

mod support;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::Env;

fn start(id: &str) -> String {
    json!({ "session_id": id, "transcript_path": "/tmp/t.jsonl", "cwd": "/tmp", "hook_event_name": "SessionStart",
            "source": "startup" })
    .to_string()
}

/// Type `leon skill hook ARGS < file` into block `b` (with `prefix` env assignments) and wait for it to
/// finish; returns how long it took.
fn hook_in_block(env: &Env, b: &str, n: &mut u32, prefix: &str, args: &str, stdin: &str) -> Duration {
    *n += 1;
    let input = env.dir.path().join(format!("in-{n}.json"));
    std::fs::write(&input, stdin).unwrap();
    let t = Instant::now();
    let line = format!("{prefix} leon skill hook {args} < '{}'; echo hooked-{n}-$?\r", input.display());
    env.ok(&["-b", b, "send", &line]);
    let done = format!("hooked-{n}-0");
    env.screen_until(b, "the hook", |s| s.lines().any(|l| l.trim() == done));
    t.elapsed()
}

#[test]
fn the_hook_records_each_agents_resume_command() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    let resume = || env.json(&["block", "inspect", &b, "--json"])["resume"].clone();
    let n = &mut 0;

    hook_in_block(&env, &b, n, "", "--agent claude", &start("c7583d2c-3133-4d13-b5f0-45883f1cabcb"));
    assert_eq!(resume(), json!(["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]));
    assert!(env.capture(&b).contains("You are running inside leon"), "the hook still prints the context");

    hook_in_block(&env, &b, n, "", "--agent codex", &start("019a7e2b-aaaa-7bbb-8ccc-0123456789ab"));
    let codex = json!(["codex", "resume", "019a7e2b-aaaa-7bbb-8ccc-0123456789ab"]);
    assert_eq!(resume(), codex);

    // Odd ids, broken JSON, and the old command without --agent record nothing and never fail.
    for (args, stdin) in [
        ("--agent claude", start("-x")),
        ("--agent claude", start("a;rm -rf ~")),
        ("--agent claude", start("a\u{1b}b")),
        ("--agent claude", start(&"a".repeat(200))),
        ("--agent claude", "not json".to_string()),
        ("", start("c7583d2c-3133-4d13-b5f0-45883f1cabcb")),
    ] {
        hook_in_block(&env, &b, n, "", args, &stdin);
        assert_eq!(resume(), codex, "{args} {stdin}");
    }

    // A server that never answers: the hook gives up within a few seconds and still succeeds.
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let sock = dir.path().join("server.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap(); // connects queue; no reply
    let prefix = format!("LEON_SERVER='{}'", sock.display());
    let took = hook_in_block(&env, &b, n, &prefix, "--agent claude", &start("c7583d2c"));
    assert!(took < Duration::from_secs(3), "took {took:?}");
}

#[test]
fn an_agent_with_no_terminal_records_nothing() {
    // A nested `claude -p` under an agent's Bash tool has LEON_* from the block but no terminal.
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    let s = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let mut c = env.cmd();
    c.args(["skill", "hook", "--agent", "claude"])
        .env("LEON_SESSION", &s)
        .env("LEON_BLOCK", &b)
        .env("LEON_SERVER", env.socket())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = c.spawn().unwrap();
    child.stdin.take().unwrap().write_all(start("c7583d2c-3133-4d13-b5f0-45883f1cabcb").as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success());
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], Value::Null);
}

#[test]
fn outside_leon_the_hook_is_silent_and_calls_no_server() {
    let env = Env::new();
    let mut child = env
        .cmd()
        .args(["skill", "hook", "--agent", "claude"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(start("c7583d2c-3133-4d13-b5f0-45883f1cabcb").as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success());
    assert_eq!(String::from_utf8_lossy(&o.stdout), "");
    assert!(!env.socket().exists(), "no server was started");
}

#[test]
fn the_hook_doesnt_wait_on_stdin_that_never_closes() {
    let env = Env::new();
    let mut child = env
        .cmd()
        .args(["skill", "hook", "--agent", "claude"])
        .env("LEON_SESSION", "session:x")
        .env("LEON_BLOCK", "block:x")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let _open = child.stdin.take().unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "the hook waited on open stdin for over 5 s");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn setup_upgrades_the_old_hook_command() {
    let root = tempfile::tempdir().unwrap();
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let (home, claude, codex) = (root.path().join("home"), root.path().join("claude"), root.path().join("codex"));
    for d in [&home, &claude, &codex] {
        std::fs::create_dir_all(d).unwrap();
    }
    let old = "command -v leon >/dev/null 2>&1 && leon skill hook || true";
    let settings = json!({ "hooks": { "SessionStart": [{ "hooks": [{ "type": "command", "command": old }] }] } });
    std::fs::write(claude.join("settings.json"), settings.to_string()).unwrap();
    std::fs::write(codex.join("hooks.json"), settings.to_string()).unwrap();
    let leon = |args: &[&str]| -> Value {
        let o = Command::new(env!("CARGO_BIN_EXE_leon"))
            .args(args)
            .env("HOME", &home)
            .env("CLAUDE_CONFIG_DIR", &claude)
            .env("CODEX_HOME", &codex)
            .env("LEON_RUNTIME_DIR", rt.path())
            .env("LEON_AUTOSTART", "0")
            .output()
            .unwrap();
        assert!(o.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        serde_json::from_slice(&o.stdout).unwrap_or(Value::Null)
    };
    let hook_state = |s: &Value, agent: &str| {
        s.as_array().unwrap().iter().find(|a| a["agent"] == agent).unwrap()["hook"]["state"].clone()
    };
    let s = leon(&["skill", "status", "--json"]);
    assert_eq!(hook_state(&s, "claude"), "outdated");
    assert_eq!(hook_state(&s, "codex"), "outdated");
    leon(&["skill", "setup"]);
    let s = leon(&["skill", "status", "--json"]);
    assert_eq!(hook_state(&s, "claude"), "current");
    assert_eq!(hook_state(&s, "codex"), "current");
    let cmd = |p: &std::path::Path| -> String {
        let v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        v["hooks"]["SessionStart"][0]["hooks"][0]["command"].as_str().unwrap().to_string()
    };
    assert!(cmd(&claude.join("settings.json")).contains("leon skill hook --agent claude"));
    assert!(cmd(&codex.join("hooks.json")).contains("leon skill hook --agent codex"));
}
