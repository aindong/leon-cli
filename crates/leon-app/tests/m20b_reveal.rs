//! M20b criterion 11: the switcher (⌘K) lists a hidden float or drawer and going there shows it;
//! an Activity row for a block in a hidden drawer or float shows it too.

mod m20b_support;

use gpui::{Modifiers, TestAppContext, VisualTestContext};
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

const BLOCKED: &str = "state=blocked:kind=question";

fn hidden(app: &embedded::Embedded, sid: &str, id: &str) -> bool {
    let w = view_window(app, sid);
    let all = w["floating"].as_array().unwrap().iter().chain(w["docked"].as_array().unwrap());
    all.clone().find(|o| o["block_id"] == id).is_some_and(|o| o["hidden"] == true)
}

/// A labelled float, hidden; returns its block id.
fn hidden_float(app: &embedded::Embedded, sid: &str, label: &str) -> String {
    let layout = json!({ "block": { "label": label } });
    let r = app.call("session.new_floating", json!({ "session_id": sid, "layout": layout })).unwrap();
    app.call("session.toggle", json!({ "session_id": sid, "label": label })).unwrap();
    r["block_id"].as_str().unwrap().to_string()
}

#[gpui::test]
fn the_switcher_lists_a_hidden_float_and_going_there_shows_it(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    let id = hidden_float(&app, &sid, "notes");
    let other = new_float(&app, &sid, [0.5, 0.5, 0.3, 0.3]);
    cx.run_until_parked();
    assert!(hidden(&app, &sid, &id));

    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("notes");
    // the app's model hears of the toggle on its event thread, after the call returned
    let first = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.switcher_rows().first().cloned());
    let listed = shows(cx, |cx| first(cx).is_some_and(|r| r.contains("notes") && r.contains("hidden")));
    let rows = ws.read_with(cx, |ws, _| ws.switcher_rows());
    assert!(listed, "a row for the hidden float first: {rows:?}");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(eventually(|| !hidden(&app, &sid, &id)), "going there shows it");
    assert_eq!(focused_id(&app, &sid).as_deref(), Some(id.as_str()), "and focuses it");
    assert_eq!(float_order(&app, &sid).last(), Some(&id), "on top of {other}");
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{id}"))), "the app draws it");
    app.stop();
}

#[gpui::test]
fn the_switcher_lists_a_hidden_drawer(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch" })).unwrap();
    let id = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-k");
    cx.simulate_input("scratch");
    let first = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.switcher_rows().first().cloned());
    let listed = shows(cx, |cx| first(cx).is_some_and(|r| r.contains("scratch") && r.contains("hidden")));
    let rows = ws.read_with(cx, |ws, _| ws.switcher_rows());
    assert!(listed, "{rows:?}");
    cx.simulate_keystrokes("enter");
    assert!(eventually(|| !hidden(&app, &sid, &id)), "going there shows the drawer");
    assert!(shows(cx, |cx| drawn(cx, &format!("dock-{id}"))));
    app.stop();
}

#[gpui::test]
fn an_activity_row_shows_a_hidden_drawer_or_float(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    let drawer = new_float_with_status(&app, &sid, "scratch", BLOCKED);
    app.call("session.dock_block", json!({ "session_id": sid, "block_id": drawer, "side": "bottom" })).unwrap();
    let float = new_float_with_status(&app, &sid, "agent", BLOCKED);
    assert!(eventually(|| !term(&app, &drawer).program_status().is_empty()), "the drawer's status came");
    assert!(eventually(|| !term(&app, &float).program_status().is_empty()), "the float's status came");
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch" })).unwrap();
    app.call("session.toggle", json!({ "session_id": sid, "label": "agent" })).unwrap();
    assert!(hidden(&app, &sid, &drawer) && hidden(&app, &sid, &float));

    cx.simulate_keystrokes("cmd-shift-a");
    let listed = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.activity_rows().len()) == 2);
    assert!(listed, "both hidden blocks need you");
    for want in [drawer.clone(), float.clone()] {
        let rows = ws.read_with(cx, |ws, _| ws.activity_rows());
        let i = rows.iter().position(|r| r.block_id == want).expect("its row");
        let row = at(cx, &format!("activity-row-{i}")).expect("the row is drawn");
        cx.simulate_click(center(row), Modifiers::none());
        cx.run_until_parked();
        assert!(eventually(|| !hidden(&app, &sid, &want)), "the click shows {want}");
        assert_eq!(focused_id(&app, &sid).as_deref(), Some(want.as_str()), "and focuses it");
    }
    assert_eq!(float_order(&app, &sid).last(), Some(&float), "the float is raised");
    app.stop();
}
