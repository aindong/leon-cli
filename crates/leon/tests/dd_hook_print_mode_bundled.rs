//! Review of daily-driver F10 (.agent/plans/2026-11-15-cli-docs-polish.md): short flags written
//! together (`claude -cp q`, `--continue --print`) are print mode too, and register no resume.

mod support;
use serde_json::{json, Value};
use std::os::unix::fs::PermissionsExt;
use support::Env;

#[test]
fn bundled_short_flags_are_print_mode() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let agent = env.dir.path().join("claude");
    std::fs::write(&agent, "#!/bin/sh\nin=$1; shift\nleon skill hook --agent claude < \"$in\"\n").unwrap();
    std::fs::set_permissions(&agent, std::fs::Permissions::from_mode(0o755)).unwrap();
    let input = env.dir.path().join("in.json");
    let start = json!({ "session_id": "c7583d2c-3133-4d13-b5f0-45883f1cabcb", "hook_event_name": "SessionStart", "source": "startup" });
    std::fs::write(&input, start.to_string()).unwrap();
    let b = env.ok(&["run", "--", "/bin/sh"]);
    let line = format!("'{}' '{}' -cp hi; echo hooked-$?\r", agent.display(), input.display());
    env.ok(&["-b", &b, "send", &line]);
    env.screen_until(&b, "the hook", |s| s.lines().any(|l| l.trim() == "hooked-0"));
    let resume = env.json(&["block", "inspect", &b, "--json"])["resume"].clone();
    assert_eq!(resume, Value::Null, "claude -cp is a one-shot run");
}
