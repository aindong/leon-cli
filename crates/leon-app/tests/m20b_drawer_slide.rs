//! M20b criterion 9: a drawer slides in from its edge and out to it, done within 120 ms; with
//! Reduce Motion it never slides.

mod m20b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::embedded::{self, Embedded};
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

/// After the slide: the frames the app asked for, to its end.
fn after_slide(ws: &Entity<Workspace>, cx: &mut VisualTestContext) {
    frames(ws, cx);
}

fn toggle(app: &Embedded, sid: &str, create: bool) {
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": create })).unwrap();
}

/// Show the scratch drawer, returning its id, the drawer's top when first drawn, and its top at rest.
fn show(app: &Embedded, ws: &Entity<Workspace>, cx: &mut VisualTestContext, create: bool) -> (String, f32, f32) {
    let sid = sid(ws, cx);
    toggle(app, &sid, create);
    let id = view_window(app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| drawn(cx, &format!("dock-{id}"))));
    let first = f32::from(at(cx, &format!("dock-{id}")).unwrap().origin.y);
    after_slide(ws, cx);
    let rest = f32::from(at(cx, &format!("dock-{id}")).unwrap().origin.y);
    (id, first, rest)
}

#[gpui::test]
fn a_drawer_slides_in_and_out(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let (id, first, rest) = show(&app, &ws, cx, true);
    assert!(first > rest + 20., "it starts below its place and slides up: first {first}, at rest {rest}");

    let sid = sid(&ws, cx);
    toggle(&app, &sid, false);
    assert!(in_slide_frame(&ws, cx, &format!("dock-{id}")).is_some(), "still drawn while it slides out");
    after_slide(&ws, cx);
    assert!(!drawn(cx, &format!("dock-{id}")), "gone once the slide is over");
    assert!(!ws.read_with(cx, |ws, _| ws.sliding()));

    let (_, first, rest) = show(&app, &ws, cx, false);
    assert!(first > rest + 20., "showing it again slides it in again");
    app.stop();
}

#[gpui::test]
fn reduce_motion_shows_and_hides_at_once(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    let (id, first, rest) = show(&app, &ws, cx, true);
    assert_eq!(first, rest, "no slide: drawn in place from the first frame");
    let sid = sid(&ws, cx);
    toggle(&app, &sid, false);
    assert!(shows(cx, |cx| !drawn(cx, &format!("dock-{id}"))), "hidden at once");
    assert!(!ws.read_with(cx, |ws, _| ws.sliding()));
    app.stop();
}
