//! M17 criteria 1 and 2 (.agent/plans/2026-11-15-m17-attach-tui.md): `leon attach` shows a
//! session started by `leon run`, types into it, and ctrl+b d detaches with the program still
//! running and the host terminal given back cooked.

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::Env;

#[test]
fn attach_shows_types_and_detaches() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh", "-c", "echo hello-m17; exec sh"]);
    env.screen_until(&block, "the shell's first line", |s| s.contains("hello-m17"));
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    env.ok(&["session", "rename", &session, "m17-demo"]);

    let h = Host::spawn(&env, &attach_then_stty("m17-demo"), &[]);
    h.wait_text("hello-m17");
    h.wait_for("the status bar names the session", |_| h.last_row().contains("m17-demo"));
    assert!(h.alt_screen(), "attached, the TUI draws on the alt screen");

    h.send(b"echo typed-$((6*7))\r");
    h.wait_text("typed-42");
    env.screen_until(&block, "the typed command's output in the block", |s| s.contains("typed-42"));

    h.send(b"\x02d");
    assert_restored(&h, 0);
    let b = env.json(&["block", "inspect", &block, "--json"]);
    assert!(b["exit_code"].is_null(), "detaching must leave the program running: {b}");
    env.ok(&["send", "-b", &block, "echo still-$((40+2))\r"]);
    env.screen_until(&block, "the program answering after detach", |s| s.contains("still-42"));
}
