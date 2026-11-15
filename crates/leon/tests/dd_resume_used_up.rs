//! Daily-driver criterion 6 (.agent/plans/2026-11-15-daily-driver-fixes.md): a resume command is
//! offered at one restart, not at every restart after it. Only a program that registers again
//! (the skill hook does, each time the agent starts) is offered next time.

mod support;

use serde_json::json;
use support::Env;

const TYPED: &str = "sh -c 'echo resumed-$((40+2))'";

fn start(env: &Env) {
    // bash: the restored login shell needs a line editor (dash, Linux's sh, is never offered one)
    let o = env.cmd().env("SHELL", "/bin/bash").args(["server", "start"]).output().unwrap();
    assert!(o.status.success(), "server start: {}", String::from_utf8_lossy(&o.stderr));
}

/// The screen after the last "restored from" line, once the restored shell shows it.
fn after_restore(env: &Env, b: &str, restores: usize) -> String {
    let s = env.screen_until(b, "the restored block", |s| s.matches("— restored from ").count() >= restores);
    s.rsplit("— restored from ").next().unwrap().to_string()
}

#[test]
fn an_offered_command_is_not_offered_again_at_the_next_restart() {
    let env = Env::new();
    start(&env);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    let set = json!({ "command": ["sh", "-c", "echo resumed-$((40+2))"] }).to_string();
    env.ok(&["-b", &b, "block", "call", "set_resume", &set]);
    env.ok(&["server", "stop"]);

    start(&env);
    env.screen_until(&b, "the offered command", |s| s.contains(TYPED));
    // still shown until the next restart (M14 criterion 4)
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], json!(["sh", "-c", "echo resumed-$((40+2))"]));
    env.ok(&["-b", &b, "send-key", "ctrl+c"]); // skipped
    env.ok(&["server", "stop"]);

    start(&env);
    env.ok(&["-b", &b, "send", "echo probe-$((1+1))\r"]);
    env.screen_until(&b, "the probe", |s| s.lines().any(|l| l.trim() == "probe-2"));
    let after = after_restore(&env, &b, 2);
    assert!(!after.contains(TYPED) && !after.contains("press Enter to resume"), "offered again:\n{after}");
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], serde_json::Value::Null);

    // registering again (what a resumed agent's hook does) is offered at the next restart
    env.ok(&["-b", &b, "block", "call", "set_resume", &set]);
    env.ok(&["server", "stop"]);
    start(&env);
    env.screen_until(&b, "the command offered again", |s| {
        s.rsplit("— restored from ").next().is_some_and(|a| a.contains(TYPED))
    });
}
