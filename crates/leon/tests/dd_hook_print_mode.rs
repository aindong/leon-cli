//! Daily-driver F10 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 5): a one-shot
//! `claude -p` in a pane registers no resume command. The hook runs in the foreground of a leon
//! block under a stand-in agent: a script whose argv carries the agent's flags, as a native
//! `claude -p hi` or an npm `node …/claude -p hi` does.

mod support;

use std::os::unix::fs::PermissionsExt;

use serde_json::{json, Value};
use support::Env;

const AGENT: &str = "#!/bin/sh\nin=$1; shift\nleon skill hook --agent claude < \"$in\"\n";

/// Run the stand-in agent with `flags` in block `b` and wait for it; `id` is its session id.
fn agent_in_block(env: &Env, b: &str, n: &mut u32, id: &str, flags: &str) {
    *n += 1;
    let input = env.dir.path().join(format!("in-{n}.json"));
    let start = json!({ "session_id": id, "transcript_path": "/tmp/t.jsonl", "cwd": "/tmp",
                        "hook_event_name": "SessionStart", "source": "startup" });
    std::fs::write(&input, start.to_string()).unwrap();
    let agent = env.dir.path().join("claude");
    let line = format!("'{}' '{}' {flags}; echo hooked-{n}-$?\r", agent.display(), input.display());
    env.ok(&["-b", b, "send", &line]);
    let done = format!("hooked-{n}-0");
    env.screen_until(b, "the hook", |s| s.lines().any(|l| l.trim() == done));
}

#[test]
fn print_mode_registers_no_resume() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let agent = env.dir.path().join("claude");
    std::fs::write(&agent, AGENT).unwrap();
    std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).unwrap();
    let n = &mut 0;
    let resume = |b: &str| env.json(&["block", "inspect", b, "--json"])["resume"].clone();

    let b = env.ok(&["run", "--", "/bin/sh"]);
    for flags in ["-p hi", "--print hi", "--print=text hi", "--model x -p 'one shot'"] {
        agent_in_block(&env, &b, n, "c7583d2c-3133-4d13-b5f0-45883f1cabcb", flags);
        assert_eq!(resume(&b), Value::Null, "claude {flags} is a one-shot run");
    }

    for (flags, id) in [("", "11111111-2222-4333-8444-555555555555"), ("-- -p", "66666666-7777-4888-9999-000000000000")]
    {
        agent_in_block(&env, &b, n, id, flags);
        assert_eq!(resume(&b), json!(["claude", "--resume", id]), "claude {flags} is interactive");
    }
}
