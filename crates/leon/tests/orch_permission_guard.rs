//! Agent orchestration criterion 4: while a block waits on a permission prompt (root record
//! blocked/permission), input from leon's commands and the API is refused, so an agent can't
//! approve it, nor lift the record to get around that; a client outside leon that watches the
//! block, as the app does, still types, and the block's own program reports it answered.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::{json, Value};
use support::orch::Orch;

/// The worker: echoes each line, and once the app's answer arrives reports `working` itself, as
/// its agent's PostToolUse hook would.
const WORKER: &str = "echo ready\\>; while IFS= read -r l; do echo \"GOT: $l\"; \
                      [ \"$l\" = approve ] && leon notify --state working; done";

/// Send one request on `conn` and return its reply, skipping the screen lines a watch pushes.
fn call(conn: &mut BufReader<UnixStream>, id: u64, method: &str, params: Value) -> Value {
    let line = json!({ "id": id, "method": method, "params": params }).to_string() + "\n";
    conn.get_mut().write_all(line.as_bytes()).unwrap();
    loop {
        let mut l = String::new();
        assert!(conn.read_line(&mut l).unwrap() > 0, "the server closed the connection");
        let v: Value = serde_json::from_str(&l).unwrap();
        if v["id"] == id {
            return v;
        }
    }
}

fn root_state(o: &Orch, b: &str) -> (String, String) {
    let r = o.env.json(&["block", "call", "-b", b, "program_status"]);
    let rec = &r["records"][0]["record"];
    (rec["state"].as_str().unwrap_or("").into(), rec["kind"].as_str().unwrap_or("").into())
}

#[test]
fn an_agent_cannot_answer_a_permission_prompt_but_the_app_can() {
    let o = Orch::new();
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", WORKER]);
    o.env.screen_until(&w, "the worker's prompt", |t| t.contains("ready>"));
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);

    let refused = |args: &[&str]| {
        let out = o.env.run(args);
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(!out.status.success(), "leon {args:?} was let through");
        assert!(err.contains("permission"), "leon {args:?} says why: {err}");
    };
    refused(&["send", "-b", &w, "yes"]);
    refused(&["send-key", "-b", &w, "enter"]);
    let write =
        json!({ "session_id": s, "block_id": w, "method": "dev.leon.terminal.write", "args": { "data": "y\r" } });
    refused(&["api", "call", "block.method", &write.to_string()]);
    refused(&["api", "call", "client.key", &json!({ "session_id": s, "block_id": w, "key": "y" }).to_string()]);
    refused(&["api", "call", "client.key", &json!({ "session_id": s, "key": "y" }).to_string()]); // the focused block
    let paste = json!({ "session_id": s, "block_id": w, "event": { "type": "paste", "data": "y\n" } });
    refused(&["api", "call", "block.event", &paste.to_string()]);
    refused(&["agent", "tell", &w, "yes"]);
    // nor can it lift the record first
    refused(&["notify", "-b", &w, "--state", "working"]);
    assert_eq!(root_state(&o, &w), ("blocked".into(), "permission".into()), "the record stayed");

    // nor type through a `leon attach` it runs in a pane of its own
    let sneaky = o.env.ok(&["run", "--label", "sneaky", "--", "leon", "attach", &w]);
    o.env.screen_until(&sneaky, "attach showing the worker", |t| t.contains("ready>"));
    o.env.ok(&["send", "-b", &sneaky, "approve"]);
    o.env.ok(&["send-key", "-b", &sneaky, "enter"]);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(o.count(&w, "GOT:"), 0, "nothing reached the pane:\n{}", o.env.capture(&w));
    o.env.ok(&["block", "close", &sneaky]);

    // the app's way: a connection from outside leon that watches the block types into it
    let stream = UnixStream::connect(o.env.socket()).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut app = BufReader::new(stream);
    let r = call(&mut app, 1, "block.watch", json!({ "block_ids": [w] }));
    assert!(r.get("error").is_none(), "{r}");
    let r = call(
        &mut app,
        2,
        "block.method",
        json!({ "session_id": s, "block_id": w,
        "method": "dev.leon.terminal.write", "args": { "data": "approve" } }),
    );
    assert!(r.get("error").is_none(), "the watching client is let through: {r}");
    let r = call(
        &mut app,
        3,
        "block.event",
        json!({ "session_id": s, "block_id": w,
        "event": { "type": "key", "key": "enter" } }),
    );
    assert!(r.get("error").is_none(), "{r}");
    o.env.screen_until(&w, "the app's keys", |t| t.contains("GOT: approve"));

    // its own program reported it answered: leon's commands type again
    support::orch::until("the worker's own report", 10, || (root_state(&o, &w).0 == "working").then_some(()));
    o.env.ok(&["send", "-b", &w, "after"]);
    o.env.ok(&["send-key", "-b", &w, "enter"]);
    o.env.screen_until(&w, "typing after the answer", |t| t.contains("GOT: after"));
}

#[test]
fn a_question_or_a_plain_block_is_not_guarded() {
    let o = Orch::new();
    let (_, w) = o.reader("asker");
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "question", "--body", "which db?"]);
    o.env.ok(&["send", "-b", &w, "postgres"]);
    o.env.ok(&["send-key", "-b", &w, "enter"]);
    o.env.screen_until(&w, "the answer", |t| t.contains("GOT: postgres"));
    o.env.ok(&["notify", "-b", &w, "--state", "working"]); // a question's record lifts from anywhere
}
