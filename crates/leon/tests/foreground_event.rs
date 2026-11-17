//! The program in a block's foreground, through a real server: `session.list_blocks` gives its name
//! (`foreground`), never its arguments, and each change is one `foreground_changed` event, a
//! silent program (`sleep`) included, and null once the program exited.

mod support;

use std::time::{Duration, Instant};

use serde_json::Value;
use support::Env;

fn block(env: &Env, s: &str, b: &str) -> Value {
    let all = env.json(&["api", "call", "session.list_blocks", &format!(r#"{{"session_id":"{s}"}}"#)]);
    all["blocks"].as_array().unwrap().iter().find(|x| x["block_id"] == b).cloned().expect("the block is listed")
}

/// The `foreground_changed` events for `b` in the next `quiet` (other events skipped).
fn changes(ev: &mut support::Stream, b: &str, quiet: Duration) -> Vec<Value> {
    let mut got = Vec::new();
    let end = Instant::now() + quiet;
    while let Some(line) = ev.line(end.saturating_duration_since(Instant::now())) {
        let e = support::Ev::new(&serde_json::from_str::<Value>(&line).unwrap()["event"]);
        if e.name == "foreground_changed" && e.block_id.as_deref() == Some(b) {
            got.push(e.payload["foreground"].clone());
        }
    }
    got
}

#[test]
fn a_silent_program_its_end_and_the_exit_are_each_one_event() {
    let env = Env::new();
    let b = env.ok(&["run", "--keep-open", "--", "sh"]);
    let s = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    env.screen_until(&b, "the prompt", |t| t.contains('$'));
    let mut ev = env.events(&["-s", &s]);
    let sh = block(&env, &s, &b)["foreground"].as_str().expect("the shell is in front").to_string();

    env.ok(&["-b", &b, "send", "sleep 4321\r"]);
    // over a second of silence, five ticks or more: one event
    assert_eq!(changes(&mut ev, &b, Duration::from_millis(1500)), [Value::from("sleep")]);
    let listed = block(&env, &s, &b);
    assert_eq!(listed["foreground"], "sleep");
    assert!(!listed.to_string().contains("4321"), "the name only, never the arguments: {listed}");

    env.ok(&["-b", &b, "send", "\u{3}"]);
    assert_eq!(changes(&mut ev, &b, Duration::from_millis(1500)), [Value::from(sh.as_str())]);

    env.ok(&["-b", &b, "send", "exit\r"]);
    assert_eq!(changes(&mut ev, &b, Duration::from_millis(1500)), [Value::Null], "null once it exited");
    assert_eq!(block(&env, &s, &b)["foreground"], Value::Null);
}
