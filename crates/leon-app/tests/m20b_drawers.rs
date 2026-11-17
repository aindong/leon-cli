//! M20b criterion 8: a drawer is drawn at its side and size, the tiles fill the rest, dragging its
//! divider sets its size on the server, and hiding it keeps its program running.

mod m20b_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

/// Let any slide finish: the frames the app asks for, to its end.
fn after_slide(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    frames(ws, cx);
}

fn near_px(a: gpui::Pixels, b: f32) -> bool {
    (f32::from(a) - b).abs() <= 1.5
}

#[gpui::test]
fn a_bottom_drawer_takes_its_share_and_its_divider_resizes_it(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    let tile = first_tile(&app, &sid);
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    let id = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| drawn(cx, &format!("dock-{id}"))), "the drawer is drawn");
    after_slide(&ws, cx);

    let area = at(cx, "panes-area").unwrap();
    let (ah, abottom) = (f32::from(area.size.height), f32::from(area.origin.y + area.size.height));
    let d = at(cx, &format!("dock-{id}")).unwrap();
    assert!(near_px(d.origin.y, abottom - (ah * 0.3).round()), "at the bottom, 30% high: {d:?} in {area:?}");
    assert!(near_px(d.origin.y + d.size.height, abottom - 6.), "inset from the bottom edge: {d:?}");
    let t = at(cx, &format!("pane-{tile}")).unwrap();
    assert!(t.origin.y + t.size.height <= d.origin.y, "the tile ends above the drawer: {t:?} {d:?}");

    let handle = at(cx, &format!("dock-handle-{id}")).expect("the drawer's divider");
    let from = center(handle);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=10 {
        let dy = -6. * step as f32;
        cx.simulate_mouse_move(from + point(px(0.), px(dy)), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let size = view_window(&app, &sid)["docked"][0]["size"].as_f64().unwrap();
        assert!(near(size, 0.3 - dy as f64 / ah as f64, 0.5 / ah as f64), "step {step}: size {size}");
    }
    cx.simulate_mouse_up(from, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();

    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch" })).unwrap();
    // the hide arrives by an event; then the slide out, on the frames the app asks for
    let gone = shows(cx, |cx| {
        tick(&ws, cx, std::time::Duration::from_millis(16));
        !drawn(cx, &format!("dock-{id}"))
    });
    assert!(gone, "hidden: gone once its slide ends");
    assert!(alive(&app, &id), "its program keeps running");
    app.stop();
}

#[gpui::test]
fn left_and_right_drawers_sit_at_their_sides(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    cx.simulate_keystrokes("cmd-d");
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let sid = sid(&ws, cx);
    let ids: Vec<String> =
        view_window(&app, &sid)["block_ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect();
    app.call("session.dock_block", json!({ "session_id": sid, "block_id": ids[0], "side": "left", "size": 0.2 }))
        .unwrap();
    app.call("session.dock_block", json!({ "session_id": sid, "block_id": ids[2], "side": "right", "size": 0.25 }))
        .unwrap();
    assert!(shows(cx, |cx| drawn(cx, &format!("dock-{}", ids[0])) && drawn(cx, &format!("dock-{}", ids[2]))));
    after_slide(&ws, cx);
    let area = at(cx, "panes-area").unwrap();
    let aw = f32::from(area.size.width);
    let l = at(cx, &format!("dock-{}", ids[0])).unwrap();
    let r = at(cx, &format!("dock-{}", ids[2])).unwrap();
    let mid = at(cx, &format!("pane-{}", ids[1])).unwrap();
    assert!(near_px(l.origin.x + l.size.width, f32::from(area.origin.x) + (aw * 0.2).round()), "{l:?}");
    assert!(near_px(r.origin.x, f32::from(area.origin.x) + aw - (aw * 0.25).round()), "{r:?}");
    assert!(l.origin.x + l.size.width <= mid.origin.x && mid.origin.x + mid.size.width <= r.origin.x, "{mid:?}");
    app.stop();
}
