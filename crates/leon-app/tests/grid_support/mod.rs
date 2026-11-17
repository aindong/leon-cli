//! What the phase-2 grid tests share: the M11 helpers, and a pane split beside another running a
//! script (quiet programs, never a login shell whose prompt may redraw on its own).
#![allow(dead_code)]

#[path = "../m11_support/mod.rs"]
mod m11;
pub use m11::*;

use gpui::{Entity, VisualTestContext};
use leon_app::view::Workspace;
use serde_json::json;

/// Split `anchor` and run `sh -c script` beside it, focused; returns its block once its grid drew
/// `ready`.
pub fn split_running(
    s: &Setup,
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    anchor: &str,
    script: &str,
    ready: &str,
) -> String {
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", script] } } });
    let p = json!({ "session_id": sid, "anchor_id": anchor, "direction": "horizontal", "side": "after",
                    "layout": layout, "focus": true });
    let id = s.app.call("session.new_split", p).unwrap()["block_id"].as_str().unwrap().to_string();
    let drew = |cx: &mut VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.grid_drew(&id)).is_some_and(|d| d.text.contains(ready))
    };
    assert!(shows(cx, drew), "the split never drew {ready:?}");
    id
}

/// How many times each shown pane's grid rendered.
pub fn grid_renders(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> u64 {
    ws.read_with(cx, |ws, _| ws.grid_renders().get(id).copied().unwrap_or(0))
}

/// Let `d` of real time pass with the window running (the test clock with it).
pub fn run_for(cx: &mut VisualTestContext, d: std::time::Duration) {
    let end = std::time::Instant::now() + d;
    while std::time::Instant::now() < end {
        let t = std::time::Instant::now();
        std::thread::sleep(std::time::Duration::from_millis(5));
        cx.executor().advance_clock(t.elapsed());
        cx.run_until_parked();
    }
}

/// How many times section `name` rendered (`sidebar`, `titlebar`, `header-<block>`).
pub fn section_renders(ws: &Entity<Workspace>, cx: &mut VisualTestContext, name: &str) -> u64 {
    ws.read_with(cx, |ws, _| ws.section_renders().get(name).copied().unwrap_or(0))
}

/// Whether section `name` renders again within 10 s after `change`.
pub fn renders_again(
    ws: &Entity<Workspace>,
    cx: &mut VisualTestContext,
    name: &str,
    change: impl FnOnce(&mut VisualTestContext),
) -> bool {
    cx.run_until_parked();
    let before = section_renders(ws, cx, name);
    change(cx);
    shows(cx, |cx| section_renders(ws, cx, name) > before)
}

/// A program that says it is working (OSC 7501), then stays.
pub const WORKING: &str = r"stty -echo; printf '\033]7501;state=working\007'; echo w-ready; exec sleep 300";
/// A program that prints its marker and stays quiet.
pub fn quiet(marker: &str) -> String {
    format!("stty -echo; echo {marker}; exec sleep 300")
}
