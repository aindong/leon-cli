//! M17 criterion 6: the prefix keys for tabs, zoom, focus and the picker, and prefix twice
//! typing the prefix itself.

mod support;

use serde_json::Value;
use support::host::Host;
use support::Env;

fn view(env: &Env, session: &str) -> Value {
    env.view(session)
}

fn active(v: &Value) -> usize {
    v["windows"].as_array().unwrap().iter().position(|w| w["active"] == true).unwrap()
}

#[test]
fn prefix_keys_drive_tabs_zoom_focus_and_the_picker() {
    let env = Env::new();
    let first = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));

    h.send(b"\x02c");
    Host::until("a second tab, active", || {
        let v = view(&env, &session);
        v["windows"].as_array().unwrap().len() == 2 && active(&v) == 1
    });
    h.send(b"\x02p");
    Host::until("the first tab active again", || active(&view(&env, &session)) == 0);
    h.send(b"\x02n");
    Host::until("the second tab active", || active(&view(&env, &session)) == 1);
    h.send(b"\x02p");
    Host::until("back on the first tab", || active(&view(&env, &session)) == 0);

    h.send(b"\x02%");
    Host::until("two panes", || env.window(&session, 0)["layout"]["split"].is_object());
    let right = env.window(&session, 0)["focused_block_id"].as_str().unwrap().to_string();
    assert_ne!(right, first, "the new pane takes focus");
    h.send(b"\x02\x1b[D"); // prefix, left arrow
    Host::until("focus moves left", || env.window(&session, 0)["focused_block_id"] == first.as_str());
    h.send(b"\x02l");
    Host::until("focus moves right with l", || env.window(&session, 0)["focused_block_id"] == right.as_str());
    h.send(b"\x02h");
    Host::until("focus moves left with h", || env.window(&session, 0)["focused_block_id"] == first.as_str());

    h.send(b"\x02z");
    Host::until("the focused pane zoomed", || env.window(&session, 0)["zoomed_block_id"] == first.as_str());
    h.send(b"\x02z");
    Host::until("zoom undone", || env.window(&session, 0)["zoomed_block_id"].is_null());

    // prefix twice types the prefix: ctrl+b shows nothing in sh, so `od` prints the byte
    h.send(b"od -An -tx1\r");
    h.send(b"\x02\x02\r\x04");
    h.wait_for("od shows the ctrl+b byte", |s| s.contains(" 02 0a"));

    h.send(b"\x02s");
    h.wait_for("the picker", |s| s.contains("Pick a session, tab or block"));
    h.send(b"\x1b"); // esc goes back to the session it came from
    h.wait_for("attached again", |s| !s.contains("Pick a session") && h.last_row().contains("leon"));
}
