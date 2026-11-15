//! What the M20b view tests share: the M10b helpers, plus floats and drawers made and read
//! through the server, as the CLI would.
#![allow(dead_code)]

#[path = "../m10b_support/mod.rs"]
mod m10b;
pub use m10b::*;

use std::time::{Duration, Instant};

use gpui::{Bounds, Entity, Pixels, VisualTestContext};
use leon_app::embedded::Embedded;
use leon_app::view::{self, Workspace};
use serde_json::{json, Value};

/// The selected session's id.
pub fn sid(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> String {
    ws.read_with(cx, |ws, _| ws.session_id()).expect("a session is selected")
}

/// The active window as `session.view` shows it (what the CLI and attach see).
pub fn view_window(app: &Embedded, sid: &str) -> Value {
    let v = app.call("session.view", json!({ "session_id": sid })).unwrap();
    v["windows"].as_array().unwrap().iter().find(|w| w["active"] == true).unwrap().clone()
}

/// Open a floating shell at `bounds` [x, y, w, h]; returns its block id.
pub fn new_float(app: &Embedded, sid: &str, bounds: [f64; 4]) -> String {
    let b = json!({ "x": bounds[0], "y": bounds[1], "width": bounds[2], "height": bounds[3] });
    let r = app.call("session.new_floating", json!({ "session_id": sid, "bounds": b })).unwrap();
    r["block_id"].as_str().unwrap().to_string()
}

/// Open a labelled float whose program reports OSC 7501 `status` (`state=blocked`, ...) and stays;
/// returns its block id. No shell, so nothing is typed before a prompt is ready.
pub fn new_float_with_status(app: &Embedded, sid: &str, label: &str, status: &str) -> String {
    let script = format!("printf '\\033]7501;{status}\\007'; exec sleep 300");
    let command = json!(["/bin/sh", "-c", script]);
    let layout = json!({ "block": { "label": label, "options": { "command": command } } });
    let r = app.call("session.new_floating", json!({ "session_id": sid, "layout": layout })).unwrap();
    r["block_id"].as_str().unwrap().to_string()
}

/// A float's bounds in the server's view, [x, y, w, h].
pub fn float_bounds(app: &Embedded, sid: &str, id: &str) -> Option<[f64; 4]> {
    let w = view_window(app, sid);
    let f = w["floating"].as_array()?.iter().find(|f| f["block_id"] == id)?.clone();
    let n = |k: &str| f["bounds"][k].as_f64().unwrap();
    Some([n("x"), n("y"), n("width"), n("height")])
}

/// The floats' block ids back to front.
pub fn float_order(app: &Embedded, sid: &str) -> Vec<String> {
    let w = view_window(app, sid);
    w["floating"].as_array().unwrap().iter().map(|f| f["block_id"].as_str().unwrap().to_string()).collect()
}

pub fn focused_id(app: &Embedded, sid: &str) -> Option<String> {
    view_window(app, sid)["focused_block_id"].as_str().map(String::from)
}

/// Wait for a drawer to start sliding, then give `sel`'s bounds in that very frame (None: it isn't
/// drawn). `shows` sleeps before each look, and on a busy machine that sleep outlasts the 120 ms
/// slide, so the look would be at a later frame.
pub fn in_slide_frame(ws: &Entity<Workspace>, cx: &mut VisualTestContext, sel: &str) -> Option<Bounds<Pixels>> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        if ws.read_with(cx, |ws, _| ws.sliding()) {
            break;
        }
        assert!(Instant::now() < end, "no drawer started sliding");
        std::thread::sleep(Duration::from_millis(20));
    }
    view::drawn().contains(sel).then(|| cx.debug_bounds(Box::leak(sel.to_string().into_boxed_str()))).flatten()
}

/// The first tiled block of the active window.
pub fn first_tile(app: &Embedded, sid: &str) -> String {
    let w = view_window(app, sid);
    w["block_ids"][0].as_str().unwrap().to_string()
}

/// Whether the block's program still runs (its terminal is alive in the server).
pub fn alive(app: &Embedded, id: &str) -> bool {
    app.ctx().state().block(id).is_ok_and(|(_, b)| b.exit_code.is_none())
}

pub fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}
