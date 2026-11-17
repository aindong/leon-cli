//! What the layout-motion tests share (motion PR 2): the grid tests' helpers, a pane that counts the
//! SIGWINCHes its program gets, a tile's bounds, and the API calls that change a layout.
#![allow(dead_code)]

#[path = "../grid_support/mod.rs"]
mod grid;
pub use grid::*;

use std::time::Duration;

use gpui::{Entity, VisualTestContext};
use leon_app::view::Workspace;
use serde_json::json;

/// A display frame's step of GPUI's clock.
pub const FRAME: Duration = Duration::from_millis(16);

/// A program that prints `W` on a line of its own for each SIGWINCH (a PTY resize), after `marker`.
pub fn winches(marker: &str) -> String {
    format!("stty -echo; trap 'echo W' WINCH; echo {marker}; while :; do sleep 0.05; done")
}

/// How many SIGWINCHes pane `id`'s program printed.
pub fn winched(s: &Setup, id: &str) -> usize {
    term(&s.app, id).text(true).lines().filter(|l| l.trim() == "W").count()
}

/// Tile `id`'s (x, y, w, h) as last drawn, if drawn.
pub fn tile(cx: &mut VisualTestContext, id: &str) -> Option<(f32, f32, f32, f32)> {
    at(cx, &format!("pane-{id}"))
        .map(|b| (f32::from(b.origin.x), f32::from(b.origin.y), f32::from(b.size.width), f32::from(b.size.height)))
}

/// Whether `b` lies strictly between `a` and `c` on width (the axis a side-by-side split moves).
pub fn between_w(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32), c: (f32, f32, f32, f32)) -> bool {
    (a.2.min(c.2) + 0.5..a.2.max(c.2) - 0.5).contains(&b.2)
}

/// Frames the app asks for until it asks for none; how many.
pub fn to_rest(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> usize {
    frames(ws, cx)
}

/// Wait until a layout motion is on its way (a change arrives by an event).
pub fn moving(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> bool {
    shows(cx, |cx| ws.read_with(cx, |ws, _| ws.animating()))
}

pub fn sid(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> String {
    ws.read_with(cx, |ws, _| ws.session_id()).expect("a session")
}

/// Split `anchor` to the right with a quiet program (focus stays), without waiting for it to draw.
pub fn split_quiet(s: &Setup, sid: &str, anchor: &str, marker: &str) -> String {
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", quiet(marker)] } } });
    let p = json!({ "session_id": sid, "anchor_id": anchor, "direction": "horizontal", "side": "after",
                    "layout": layout, "focus": false });
    s.app.call("session.new_split", p).unwrap()["block_id"].as_str().unwrap().to_string()
}

pub fn close(s: &Setup, sid: &str, id: &str) {
    s.app.call("block.close", json!({ "session_id": sid, "block_id": id })).unwrap();
}

pub fn zoom(s: &Setup, sid: &str, id: &str, on: bool) {
    s.app.call("session.zoom_block", json!({ "session_id": sid, "block_id": id, "zoom": on })).unwrap();
}
