//! ADR R3/R4 (docs/architecture/gpui.md), phase 3: the program in a pane (its foreground, #32) is
//! in its header's input, so a foreground change renders the header again and leaves the grid,
//! which drew nothing new, alone.
mod grid_support;

use gpui::TestAppContext;
use grid_support::*;

/// A shell that, on a newline it doesn't echo, becomes `sleep` without printing anything.
const BECOMES: &str = "stty -echo; echo p-ready; read a; exec sleep 300";

#[gpui::test]
fn a_foreground_change_renders_the_header_not_the_grid(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, BECOMES, "p-ready");
    let shell = format!("header-program-{id}-shell");
    assert!(shows(cx, |cx| drawn(cx, &shell)), "the header shows a shell's glyph");
    cx.run_until_parked();
    let (header, grid) = (section_renders(&ws, cx, &format!("header-{id}")), grid_renders(&ws, cx, &id));
    term(&s.app, &id).write(b"\n").unwrap();
    let generic = format!("header-program-{id}-generic");
    // the drawn record, not `drawn`: that draws every view again (a cached one's bounds), grids too
    let changed = shows(cx, |cx| {
        cx.run_until_parked();
        leon_app::view::drawn().contains(&generic)
    });
    let (header, grid) =
        (section_renders(&ws, cx, &format!("header-{id}")) - header, grid_renders(&ws, cx, &id) - grid);
    s.app.stop();
    assert!(changed, "the header shows the new program's glyph");
    assert!(header >= 1, "the header rendered again");
    assert_eq!(grid, 0, "the grid, with nothing new on screen, did not");
}
