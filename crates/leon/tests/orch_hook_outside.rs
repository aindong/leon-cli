//! Agent orchestration criterion 6: the status hooks never fail or stall the agent and do nothing
//! outside leon: no block, or no server, means exit 0 at once, no output, and no server started.

mod support;

use std::io::Write;
use std::process::Stdio;
use std::time::{Duration, Instant};

use support::orch::private;
use support::Env;

fn hook(env: &Env, block: Option<&str>, on: &str, payload: &str) -> (i32, String, Duration) {
    let home = env.dir.path().join("home");
    let mut c = private(env.cmd(), &home);
    c.args(["skill", "hook", "--agent", "claude", "--on", on]);
    if let Some(b) = block {
        c.env("LEON_BLOCK", b).env("LEON_SESSION", "session:00000000-0000-0000-0000-000000000000");
    }
    let start = Instant::now();
    let mut child = c.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(payload.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    (out.status.code().unwrap(), text, start.elapsed())
}

#[test]
fn outside_leon_or_without_a_server_the_hook_does_nothing() {
    let env = Env::new();
    let stop = r#"{"hook_event_name":"Stop","last_assistant_message":"hi"}"#;
    for (block, on, payload) in [
        (None, "stop", stop),
        (None, "permission-request", r#"{"tool_name":"Bash","tool_input":{"command":"ls"}}"#),
        (Some("block:00000000-0000-0000-0000-000000000000"), "stop", stop),
        (Some("block:00000000-0000-0000-0000-000000000000"), "user-prompt-submit", "not json"),
        (Some("block:00000000-0000-0000-0000-000000000000"), "post-tool-use", ""),
    ] {
        let (code, text, took) = hook(&env, block, on, payload);
        assert_eq!(code, 0, "{on} in {block:?}: {text}");
        assert_eq!(text, "", "{on} in {block:?} prints nothing");
        assert!(took < Duration::from_secs(3), "{on} in {block:?} took {took:?}");
    }
    assert!(!env.socket().exists(), "no server was started");
}
