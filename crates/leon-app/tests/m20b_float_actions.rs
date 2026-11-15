//! M20b criterion 10: the float and drawer actions are in the palette with default keys, and each
//! does what it says to the focused pane.

mod m20b_support;

use gpui::TestAppContext;
use leon_app::embedded;
use leon_app::keymap::DEFAULTS;
use m20b_support::*;

const TITLES: [(&str, &str); 7] = [
    ("pane.float", "Float Pane"),
    ("pane.new_floating", "New Floating Pane"),
    ("pane.unfloat", "Unfloat Pane"),
    ("pane.scratch.toggle", "Show or Hide the Scratch Drawer"),
    ("pane.dock.left", "Dock Pane Left"),
    ("pane.dock.right", "Dock Pane Right"),
    ("pane.dock.bottom", "Dock Pane at the Bottom"),
];

#[test]
fn every_action_has_a_title_and_all_but_new_floating_a_default_key() {
    for (name, title) in TITLES {
        assert!(leon_app::palette::BUILTINS.contains(&(name, title)), "{name} titled {title:?} in the palette");
        if name != "pane.new_floating" {
            assert!(DEFAULTS.iter().any(|(_, a)| *a == name), "{name} has a default key");
        }
    }
}

fn floats(app: &embedded::Embedded, sid: &str) -> Vec<String> {
    let w = view_window(app, sid);
    w["floating"].as_array().unwrap().iter().map(|f| f["block_id"].as_str().unwrap().to_string()).collect()
}

fn dock_of(app: &embedded::Embedded, sid: &str, id: &str) -> Option<(String, bool)> {
    let w = view_window(app, sid);
    let d = w["docked"].as_array()?.iter().find(|d| d["block_id"] == id)?.clone();
    Some((d["side"].as_str()?.to_string(), d["hidden"] == true))
}

#[gpui::test]
fn the_actions_float_unfloat_dock_and_toggle(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows_for("float"));
    for t in ["Float Pane", "Unfloat Pane", "New Floating Pane"] {
        assert!(rows.iter().any(|(r, _)| r == t), "{t} in {rows:?}");
    }
    let sid = sid(&ws, cx);
    cx.simulate_keystrokes("cmd-d");
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let focused = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.target_block()).unwrap();

    let a = focused(cx);
    act(&ws, cx, "pane.float");
    assert!(eventually(|| floats(&app, &sid) == [a.clone()]), "pane.float floats the focused pane");
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{a}"))));
    act(&ws, cx, "pane.unfloat");
    assert!(eventually(|| floats(&app, &sid).is_empty()), "pane.unfloat puts it back");
    assert!(view_window(&app, &sid)["layout"].to_string().contains(&a), "in the tiles");

    act(&ws, cx, "pane.new_floating");
    assert!(eventually(|| floats(&app, &sid).len() == 1), "pane.new_floating opens a float");
    let new = floats(&app, &sid)[0].clone();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(new.clone())), "focused");
    act(&ws, cx, "pane.unfloat");
    assert!(eventually(|| floats(&app, &sid).is_empty()));

    for (action, side) in [("pane.dock.left", "left"), ("pane.dock.right", "right"), ("pane.dock.bottom", "bottom")] {
        let tiles = view_window(&app, &sid)["layout"].clone();
        let id = tiles.to_string();
        let target = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()).is_some_and(|t| id.contains(&t)));
        assert!(target, "a tiled pane is focused before {action}");
        let b = focused(cx);
        act(&ws, cx, action);
        assert!(eventually(|| dock_of(&app, &sid, &b) == Some((side.into(), false))), "{action} docks {b}");
        // focus a tile again for the next one
        let tile = first_tile(&app, &sid);
        app.call("session.focus_block", serde_json::json!({ "session_id": sid, "block_id": tile })).unwrap();
        cx.run_until_parked();
    }

    // the scratch drawer (on the bottom, so the docked one goes back first): made, hidden, shown
    let bottom = view_window(&app, &sid)["docked"].as_array().unwrap().iter().find(|d| d["side"] == "bottom").unwrap()
        ["block_id"]
        .clone();
    app.call("session.unfloat_block", serde_json::json!({ "session_id": sid, "block_id": bottom })).unwrap();
    act(&ws, cx, "pane.scratch.toggle");
    let scratch = || {
        let w = view_window(&app, &sid);
        let st = app.ctx().state();
        let s = st.session(&sid).unwrap();
        let b = s.blocks.iter().find(|b| b.label.as_deref() == Some("scratch"))?.id.clone();
        drop(st);
        let hidden = w["docked"].as_array()?.iter().chain(w["floating"].as_array()?).find(|d| d["block_id"] == b)?
            ["hidden"]
            == true;
        Some((b, hidden))
    };
    assert!(eventually(|| scratch().is_some_and(|(_, h)| !h)), "made and shown");
    act(&ws, cx, "pane.scratch.toggle");
    assert!(eventually(|| scratch().is_some_and(|(_, h)| h)), "hidden");
    act(&ws, cx, "pane.scratch.toggle");
    assert!(eventually(|| scratch().is_some_and(|(_, h)| !h)), "shown again");
    app.stop();
}
