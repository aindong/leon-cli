//! What the overlay focus tests share (GPUI phase 6): who has the window's focus, and typed text
//! reaching the focused pane's shell.
#![allow(dead_code)]

#[path = "../close_support/mod.rs"]
mod close;
pub use close::*;

use gpui::{Entity, VisualTestContext};
use leon_app::view::Workspace;

/// Who has the window's focus: `pane` (the workspace's own handle, which the terminal's keys go
/// through), an overlay's name (`palette`, `overview`, `find`, `rename`, `popover`), or None.
pub fn owner(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<&'static str> {
    cx.run_until_parked();
    cx.update(|window, cx| ws.read(cx).focus_owner(window, cx))
}

/// Type `echo TAG` and Enter as keys, and whether the pane the window shows now printed TAG on a
/// line of its own: the keys reached the pane.
pub fn types_into_pane(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext, tag: &str) -> bool {
    let keys: Vec<String> =
        "echo ".chars().chain(tag.chars()).map(|c| if c == ' ' { "space".into() } else { c.to_string() }).collect();
    cx.simulate_keystrokes(&keys.join(" "));
    cx.simulate_keystrokes("enter");
    let t = focused(&s.app, ws, cx);
    eventually(|| has_line(&t, tag))
}

/// The focus is the pane's, and typed text reaches it.
pub fn back_on_pane(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext, tag: &str) -> Result<(), String> {
    match owner(ws, cx) {
        Some("pane") => {}
        other => return Err(format!("the focus is {other:?}, not the pane's")),
    }
    if !types_into_pane(s, ws, cx, tag) {
        return Err(format!("typed text never reached the pane ({tag})"));
    }
    Ok(())
}
