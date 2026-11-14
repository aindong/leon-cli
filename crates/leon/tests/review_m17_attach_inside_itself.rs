//! M17 review (skeptic): `leon attach` run in a pane of the session it attaches to must not size
//! its own pane (each resize shrinks its terminal, which asks for a smaller pane again, down to
//! one row), nor draw its own pane into itself.

mod support;

use serde_json::Value;
use support::host::Host;
use support::Env;

#[test]
fn attach_inside_its_own_session_leaves_its_pane_alone() {
    let env = Env::new();
    let other = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let leon = env!("CARGO_BIN_EXE_leon");
    let me = env.ok(&["-s", &session, "split", "-b", &other, "--keep-open", "--", leon, "attach", &session]);
    let size = |b: &str| {
        let v: Value = env.json(&["-b", b, "block", "call", "size"]);
        (v["rows"].as_u64().unwrap(), v["cols"].as_u64().unwrap())
    };
    env.screen_until(&me, "the client's status bar", |s| s.contains(" leon "));
    // the other pane is fitted to the client's terminal: its rows less the status bar
    let mine = size(&me);
    Host::until("the other pane fitted", || size(&other).0 == mine.0 - 1);
    // a few more frames: typing in the other pane redraws the client
    env.ok(&["send", "-b", &other, "echo again\r"]);
    env.screen_until(&me, "the echo drawn", |s| s.contains("again"));
    assert_eq!(size(&me), mine, "the client must not resize its own pane");
    let s = env.capture(&me);
    assert!(s.contains("this terminal"), "its own pane shows a placeholder, not itself:\n{s}");
}
