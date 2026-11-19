//! Agent orchestration criterion 4: while a block waits on a permission prompt (root record
//! blocked/permission), a program inside a leon terminal (where agents run) can't type into it,
//! however it tries, nor lift the record to get around that. A person outside leon still answers:
//! `leon attach` in a plain terminal here, a client outside leon as the app is; then the block's
//! own program reports it answered and leon's commands type again.

mod support;

use serde_json::json;
use support::host::{attach_then_stty, Host};
use support::orch::{until, Orch};

/// The worker: echoes each line, and once the person's answer arrives reports `working` itself,
/// as its agent's PostToolUse hook would.
const WORKER: &str = "echo ready\\>; while IFS= read -r l; do echo \"GOT: $l\"; \
                      [ \"$l\" = approve ] && leon notify --state working; done";

fn root_state(o: &Orch, b: &str) -> (String, String) {
    let r = o.env.json(&["block", "call", "-b", b, "program_status"]);
    let rec = &r["records"][0]["record"];
    (rec["state"].as_str().unwrap_or("").into(), rec["kind"].as_str().unwrap_or("").into())
}

#[test]
fn an_agent_in_leon_cannot_answer_a_permission_prompt_but_a_person_can() {
    let o = Orch::new();
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", WORKER]);
    o.env.screen_until(&w, "the worker's prompt", |t| t.contains("ready>"));
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);

    // an agent in a pane beside it (W keeps the focus) tries leon's commands and API
    let out = o.env.dir.path().join("tries");
    let write =
        json!({ "session_id": s, "block_id": w, "method": "dev.leon.terminal.write", "args": { "data": "y\r" } });
    let key = json!({ "session_id": s, "block_id": w, "key": "y" });
    let focused = json!({ "session_id": s, "key": "y" });
    let paste = json!({ "session_id": s, "block_id": w, "event": { "type": "paste", "data": "y\n" } });
    let tries = [
        format!("leon send -b {w} yes"),
        format!("leon send-key -b {w} enter"),
        format!("leon api call block.method '{write}'"),
        format!("leon api call client.key '{key}'"),
        format!("leon api call client.key '{focused}'"),
        format!("leon api call block.event '{paste}'"),
        format!("leon agent tell {w} yes --for 2s"),
        format!("leon notify -b {w} --state working"),
    ];
    let body = tries
        .iter()
        .map(|t| format!("{t} > /dev/null 2>> '{0}.err'; echo $? >> '{0}.codes'", out.display()))
        .collect::<Vec<_>>()
        .join("\n");
    let agent = o.script("sneaky-agent", &format!("{body}\necho tried\nexec sleep 300"));
    let sneaky = o.env.ok(&["split", "-b", &w, "--focus=false", "--", agent.to_str().unwrap()]);
    o.env.screen_until(&sneaky, "the agent's tries", |t| t.contains("tried"));
    let codes = std::fs::read_to_string(format!("{}.codes", out.display())).unwrap();
    let codes: Vec<&str> = codes.lines().collect();
    assert_eq!(codes.len(), tries.len());
    for (t, c) in tries.iter().zip(&codes) {
        assert_ne!(*c, "0", "let through: {t}");
    }
    let err = std::fs::read_to_string(format!("{}.err", out.display())).unwrap();
    assert!(err.matches("permission prompt").count() >= tries.len(), "each says why:\n{err}");
    assert_eq!(root_state(&o, &w), ("blocked".into(), "permission".into()), "the record stayed");

    // nor through a `leon attach` it runs in a pane of its own
    let attached = o.env.ok(&["run", "--label", "attached", "--", "leon", "attach", &w]);
    // it can't even move the focus onto the held pane (refused too), so it never shows it: it
    // shows its own tab, and nothing typed into it could reach the prompt
    o.env.screen_until(&attached, "attach up", |t| t.contains("this terminal: leon attach"));
    assert!(!o.env.capture(&attached).contains("ready>"), "attach showed the held pane");
    assert_eq!(o.count(&w, "GOT:"), 0, "nothing reached the pane:\n{}", o.env.capture(&w));
    o.env.ok(&["block", "close", &attached]);

    // a person in a plain terminal answers through `leon attach`
    o.env.ok(&["session", "rename", &s, "work"]);
    let h = Host::spawn(&o.env, &attach_then_stty("work"), &[]);
    h.wait_text("ready>");
    h.send(b"approve\r");
    o.env.screen_until(&w, "the person's answer", |t| t.contains("GOT: approve"));
    h.send(b"\x02d");

    // its own program reported it answered: an agent's commands type again
    until("the worker's own report", 10, || (root_state(&o, &w).0 == "working").then_some(()));
    let inside = o.env.dir.path().join("after");
    let again = o.script(
        "polite-agent",
        &format!(
            "leon send -b {w} after && leon send-key -b {w} enter; echo $? > '{}'; exec sleep 300",
            inside.display()
        ),
    );
    o.env.ok(&["split", "-b", &w, "--focus=false", "--", again.to_str().unwrap()]);
    o.env.screen_until(&w, "typing after the answer", |t| t.contains("GOT: after"));
}

#[test]
fn a_question_or_a_plain_block_is_not_guarded() {
    let o = Orch::new();
    let (_, w) = o.reader("asker");
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "question", "--body", "which db?"]);
    let agent = o
        .script("answering-agent", &format!("leon send -b {w} postgres && leon send-key -b {w} enter; exec sleep 300"));
    o.env.ok(&["split", "-b", &w, "--focus=false", "--", agent.to_str().unwrap()]);
    o.env.screen_until(&w, "the answer", |t| t.contains("GOT: postgres"));
}
