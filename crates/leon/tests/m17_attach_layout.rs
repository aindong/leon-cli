//! M17 criteria 4 and 5: prefix % and " split the focused pane, the TUI draws the split with a
//! border, and a host resize refits the panes.

mod support;

use serde_json::Value;
use support::host::Host;
use support::Env;

fn size(env: &Env, block: &str) -> (u64, u64) {
    let v: Value = env.json(&["-b", block, "block", "call", "size"]);
    (v["rows"].as_u64().unwrap(), v["cols"].as_u64().unwrap())
}

fn layout(env: &Env, session: &str) -> Value {
    env.window(session, 0)["layout"].clone()
}

#[test]
fn prefix_splits_show_two_panes_with_a_border() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));

    h.send(b"\x02%");
    Host::until("a horizontal split", || layout(&env, &session)["split"]["direction"] == "horizontal");
    let tree = layout(&env, &session);
    assert!(tree["split"]["before"]["block"]["block_id"] == block.as_str(), "the old pane stays first: {tree}");
    // every pane row has the border between the two halves; the status bar row doesn't
    let s = h.wait_for("a vertical border on each pane row", |s| {
        let rows: Vec<&str> = s.lines().collect();
        rows.len() == 24 && rows[..23].iter().all(|r| r.contains('│'))
    });
    assert!(!s.lines().last().unwrap().contains('│'), "the status bar has no border:\n{s}");

    h.send(b"\x02\"");
    Host::until("a vertical split inside the right pane", || {
        layout(&env, &session)["split"]["after"]["split"]["direction"] == "vertical"
    });
    h.wait_for("a horizontal border", |s| s.contains('─'));
}

#[test]
fn a_host_resize_refits_the_pane() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    Host::until("the pane fits 23×80 (one row for the status bar)", || size(&env, &block) == (23, 80));

    h.term.resize(30, 100).unwrap();
    Host::until("the pane refits to 29×100", || size(&env, &block) == (29, 100));
    h.wait_for("the status bar on the new last row", |s| {
        s.lines().count() == 30 && s.lines().last().unwrap().contains("leon")
    });
}
