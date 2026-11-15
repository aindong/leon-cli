//! M20b follow-up: a tab's title comes from its tiled panes (the last one focused), never from a
//! float or drawer that has the focus; the overlay's own header keeps its title.

mod m20b_support;

use gpui::TestAppContext;
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_focused_float_or_drawer_leaves_the_tab_title_to_the_tiles(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    let first = first_tile(&app, &sid);
    let block = |argv: &[&str]| json!({ "block": { "options": { "command": argv } } });
    app.call(
        "session.new_split",
        json!({ "session_id": sid, "anchor_id": first, "direction": "horizontal", "side": "after",
                    "layout": block(&["/usr/bin/tail", "-f", "/dev/null"]), "focus": true }),
    )
    .unwrap();
    let titles = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.tab_titles());
    // the focused tile's program (not the first tile's, which the server falls back to)
    assert!(shows(cx, |cx| titles(cx) == ["tail"]), "{:?}", titles(cx));
    let tab = titles(cx);

    let float = app
        .call("session.new_floating", json!({ "session_id": sid, "layout": block(&["/bin/sleep", "300"]) }))
        .unwrap()["block_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(eventually(|| focused_id(&app, &sid).as_deref() == Some(float.as_str())), "the float has the focus");
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{float}"))), "the float is drawn");
    ws.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    assert_eq!(titles(cx), tab, "a focused float doesn't rename its tab");

    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    let drawer = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    assert!(eventually(|| focused_id(&app, &sid).as_deref() == Some(drawer.as_str())), "the drawer has the focus");
    assert!(shows(cx, |cx| drawn(cx, &format!("dock-{drawer}"))), "the drawer is drawn");
    assert_eq!(titles(cx), tab, "a focused drawer doesn't rename its tab");
    app.stop();
}
