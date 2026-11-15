//! A pane that joined another session keeps the `LEON_SESSION` it started with (a running
//! program's env can't change), so commands run in it go by `LEON_BLOCK`: `leon split`, `leon run`
//! and the agent hook's resume registration act on the session that holds the block now.

mod support;

use serde_json::{json, Value};
use support::Env;

fn blocks(env: &Env, sid: &str) -> Vec<Value> {
    env.json(&["-s", sid, "api", "call", "session.list_blocks"])["blocks"].as_array().cloned().unwrap_or_default()
}

fn has_label(env: &Env, sid: &str, label: &str) -> bool {
    blocks(env, sid).iter().any(|b| b["label"] == label)
}

#[test]
fn a_joined_pane_acts_on_the_session_that_holds_it() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let home = env.ok(&["new", "home"]);
    let anchor = blocks(&env, &home)[0]["block_id"].as_str().unwrap().to_string();
    let away = env.ok(&["new", "away"]);
    // its own tab in `away`, so `away` lives on after the pane leaves
    let agent = env.ok(&["-s", &away, "run", "--label", "agent", "--", "/bin/sh"]);
    let p = json!({ "session_id": home, "block_id": agent, "anchor_block_id": anchor }).to_string();
    env.ok(&["api", "call", "session.move_block", &p]);
    assert!(has_label(&env, &home, "agent"), "the pane joined home");

    let hook = env.dir.path().join("hook.json");
    let start = json!({ "session_id": "c7583d2c-3133-4d13-b5f0-45883f1cabcb", "transcript_path": "/tmp/t.jsonl",
                        "cwd": "/tmp", "hook_event_name": "SessionStart", "source": "startup" });
    std::fs::write(&hook, start.to_string()).unwrap();
    let line = format!(
        "leon split --label sp -- sleep 300 >/dev/null; leon run --label rn -- sleep 300 >/dev/null; \
         leon skill hook --agent claude < '{}' >/dev/null; echo joined-$?\r",
        hook.display()
    );
    env.ok(&["-b", &agent, "send", &line]);
    env.screen_until(&agent, "the commands", |s| s.lines().any(|l| l.trim().starts_with("joined-")));

    assert!(has_label(&env, &home, "sp"), "leon split opened beside the pane, in home");
    assert!(has_label(&env, &home, "rn"), "leon run opened a tab in home, the pane's session now");
    assert!(!has_label(&env, &away, "rn"), "not in away, the stale LEON_SESSION");
    let resume = env.json(&["block", "inspect", &agent, "--json"])["resume"].clone();
    assert_eq!(resume, json!(["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]), "the hook recorded");
}
