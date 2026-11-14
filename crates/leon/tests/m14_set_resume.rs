//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 8):
//! `dev.leon.terminal.set_resume` records, shows (`block inspect`), clears and validates a block's
//! resume command.

mod support;

use serde_json::json;
use support::Env;

#[test]
fn set_resume_records_shows_clears_and_refuses_bad_commands() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    let set = |args: serde_json::Value| env.run(&["-b", &b, "block", "call", "set_resume", &args.to_string()]);

    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], serde_json::Value::Null);
    assert!(set(json!({ "command": ["claude", "--resume", "abc-1"] })).status.success());
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], json!(["claude", "--resume", "abc-1"]));
    let text = env.ok(&["block", "inspect", &b]);
    assert!(text.lines().any(|l| l.trim() == "resume: claude --resume abc-1"), "{text}");

    let quoted = json!(["sh", "-c", "echo it's"]);
    assert!(set(json!({ "command": quoted })).status.success());
    let text = env.ok(&["block", "inspect", &b]);
    assert!(text.lines().any(|l| l.trim() == r"resume: sh -c 'echo it'\''s'"), "shell-quoted: {text}");

    let long_arg = "x".repeat(9000);
    let many: Vec<String> = (0..65).map(|i| i.to_string()).collect();
    for bad in [
        json!({ "command": [] }),
        json!({ "command": [""] }),
        json!({ "command": ["claude", "--resume", "a\rb"] }),
        json!({ "command": ["claude", "\u{1b}[2J"] }),
        json!({ "command": ["claude", "\u{9b}"] }),
        json!({ "command": ["claude", long_arg] }),
        json!({ "command": many }),
    ] {
        let o = set(bad.clone());
        assert_eq!(o.status.code(), Some(1), "refused: {bad}");
        assert!(String::from_utf8_lossy(&o.stderr).contains("bad_request"), "as a bad request: {bad}");
    }
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], quoted, "a refused call changes nothing");

    assert!(set(json!({ "command": null })).status.success());
    assert_eq!(env.json(&["block", "inspect", &b, "--json"])["resume"], serde_json::Value::Null, "null clears it");
}
