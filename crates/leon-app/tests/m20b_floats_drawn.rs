//! M20b criterion 5: a visible float is drawn over the tiles at its bounds of the pane area, with
//! a pane header (title, close); a hidden one is not drawn and its program keeps running.

mod m20b_support;

use gpui::TestAppContext;
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

fn near_px(a: gpui::Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() <= 1.
}

#[gpui::test]
fn a_float_is_drawn_over_a_split_at_its_bounds(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let sid = sid(&ws, cx);
    let low = new_float(&app, &sid, [0.1, 0.1, 0.5, 0.5]);
    // a program with a status, to see it in the float's header as a tile's shows it
    let top = new_float_with_status(&app, &sid, "agent", "state=working");
    let b = json!({ "x": 0.3, "y": 0.3, "width": 0.5, "height": 0.5 });
    app.call("session.float_block", json!({ "session_id": sid, "block_id": top, "bounds": b })).unwrap();
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{top}"))), "the top float is drawn");
    assert!(shows(cx, |cx| drawn(cx, &format!("header-status-{top}"))), "the float's header shows its status");

    let area = at(cx, "panes-area").expect("the pane area");
    let (w, h) = (f32::from(area.size.width), f32::from(area.size.height));
    for (id, b) in [(&low, [0.1, 0.1, 0.5, 0.5]), (&top, [0.3, 0.3, 0.5, 0.5])] {
        let r = at(cx, &format!("float-{id}")).unwrap_or_else(|| panic!("float {id} drawn"));
        let x = f32::from(area.origin.x) + b[0] as f32 * w;
        let y = f32::from(area.origin.y) + b[1] as f32 * h;
        assert!(near_px(r.origin.x, x) && near_px(r.origin.y, y), "{id} at {r:?}, wanted ({x}, {y})");
        assert!(near_px(r.size.width, b[2] as f32 * w) && near_px(r.size.height, b[3] as f32 * h), "{r:?}");
        assert!(drawn(cx, &format!("header-{id}")), "a float has a pane header");
        // as on a tile, an unfocused pane's controls show while the mouse is over it
        let header = at(cx, &format!("header-{id}")).unwrap();
        cx.simulate_mouse_move(center(header), None, gpui::Modifiers::none());
        assert!(drawn(cx, &format!("header-close-{id}")), "a close control");
        assert!(drawn(cx, &format!("header-unfloat-{id}")), "and one that puts it back in the tiles");
    }
    // the tiles are still there, under the floats
    let tile = first_tile(&app, &sid);
    assert!(drawn(cx, &format!("pane-{tile}")));
    app.stop();
}

#[gpui::test]
fn a_hidden_float_is_not_drawn_and_its_program_runs(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    let r = app
        .call("session.new_floating", json!({ "session_id": sid, "layout": { "block": { "label": "notes" } } }))
        .unwrap();
    let id = r["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{id}"))));
    app.call("session.toggle", json!({ "session_id": sid, "label": "notes" })).unwrap();
    assert!(shows(cx, |cx| !drawn(cx, &format!("float-{id}"))), "hidden: not drawn");
    assert!(alive(&app, &id), "its program keeps running");
    app.stop();
}
