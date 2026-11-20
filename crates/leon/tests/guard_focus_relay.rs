//! The proof this fix started from: an agent in a pane beside a held prompt asks leon to focus
//! the prompt; a person typing next would answer it. Red before the guard covered focus.

mod support;

use serde_json::json;
use support::orch::Orch;

#[test]
fn an_agent_in_leon_cannot_move_the_focus_onto_a_held_prompt() {
    let o = Orch::new();
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", "echo ready; exec sleep 300"]);
    o.env.screen_until(&w, "the worker", |t| t.contains("ready"));
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    let out = o.env.dir.path().join("focus.code");
    let f = json!({"session_id": s, "block_id": w});
    let agent = o.script(
        "focus-agent",
        &format!(
            "leon api call session.focus_block '{f}' > /dev/null 2>&1; echo $? > '{}'; echo tried; exec sleep 300",
            out.display()
        ),
    );
    // the new pane takes the focus; the agent then tries to move it back onto the held prompt
    let a = o.env.ok(&["split", "-b", &w, "--", agent.to_str().unwrap()]);
    o.env.screen_until(&a, "the agent's try", |t| t.contains("tried"));
    let code = std::fs::read_to_string(&out).unwrap();
    assert_ne!(code.trim(), "0", "PERMISSION_RELAY: a program inside leon moved the person's keys onto a held prompt");
}
