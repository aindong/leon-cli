//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 4): a block that
//! registered a resume command offers it under "ask" (the default), runs it under "always", ignores
//! it under "never"; `block inspect` shows it in every case.

mod support;

use serde_json::json;
use support::Env;

/// Start a server with `policy` set in init.lua (None: no config), register a resume command for a
/// shell block, restart, and return the env and the block.
fn restored(policy: Option<&str>) -> (Env, String) {
    let env = Env::new();
    let cfg = env.dir.path().join("init.lua");
    if let Some(p) = policy {
        std::fs::write(&cfg, format!("leon.set(\"resume\", \"{p}\")\n")).unwrap();
    }
    let start = || {
        // bash: the restored login shell needs a line editor (dash, Linux's sh, is never offered one)
        let o =
            env.cmd().env("LEON_CONFIG", &cfg).env("SHELL", "/bin/bash").args(["server", "start"]).output().unwrap();
        assert!(o.status.success(), "server start: {}", String::from_utf8_lossy(&o.stderr));
    };
    start();
    let b = env.ok(&["run", "--", "/bin/sh"]);
    let set = json!({ "command": ["sh", "-c", "echo resumed-$((40+2))"] }).to_string();
    env.ok(&["-b", &b, "block", "call", "set_resume", &set]);
    env.ok(&["server", "stop"]);
    start();
    (env, b)
}

fn resume_of(env: &Env, b: &str) -> serde_json::Value {
    env.json(&["block", "inspect", b, "--json"])["resume"].clone()
}

const TYPED: &str = "sh -c 'echo resumed-$((40+2))'";

#[test]
fn ask_types_the_command_and_waits_for_enter() {
    let (env, b) = restored(None);
    let s = env.screen_until(&b, "the offered command", |s| s.contains(TYPED));
    assert!(s.contains("press Enter to resume"), "a hint says what Enter does:\n{s}");
    assert!(!s.lines().any(|l| l.trim() == "resumed-42"), "not run before Enter:\n{s}");
    assert_eq!(resume_of(&env, &b), json!(["sh", "-c", "echo resumed-$((40+2))"]));
    env.ok(&["-b", &b, "send", "\r"]);
    env.screen_until(&b, "the resumed command", |s| s.lines().any(|l| l.trim() == "resumed-42"));
}

#[test]
fn always_runs_it() {
    let (env, b) = restored(Some("always"));
    env.screen_until(&b, "the resumed command", |s| s.lines().any(|l| l.trim() == "resumed-42"));
    assert_eq!(resume_of(&env, &b), json!(["sh", "-c", "echo resumed-$((40+2))"]));
}

#[test]
fn never_ignores_it() {
    let (env, b) = restored(Some("never"));
    env.screen_until(&b, "the restored block", |s| s.contains("— restored from "));
    env.ok(&["-b", &b, "send", "echo probe-$((1+1))\r"]);
    let s = env.screen_until(&b, "the probe", |s| s.lines().any(|l| l.trim() == "probe-2"));
    let after = s.split("— restored from ").nth(1).unwrap();
    assert!(!after.contains("resumed-42") && !after.contains(TYPED), "never typed or run:\n{s}");
    assert_eq!(resume_of(&env, &b), json!(["sh", "-c", "echo resumed-$((40+2))"]));
}
