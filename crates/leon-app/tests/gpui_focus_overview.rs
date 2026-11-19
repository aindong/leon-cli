//! GPUI phase 6 (ADR R9): the overview takes the focus while open; Escape, its key again, and
//! Enter on a card each give the focus back to the pane shown, and typed text reaches it.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

fn open_overview(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    cx.simulate_keystrokes("cmd-shift-\\");
    assert!(ws.read_with(cx, |ws, _| ws.overview_open()), "the overview opened");
    assert_eq!(owner(ws, cx), Some("overview"), "the overview has the focus");
}

/// Draw the frames its open or close asks for, to the end.
fn to_rest(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    frames(ws, cx);
}

#[gpui::test]
fn every_way_the_overview_closes_gives_the_pane_its_keys(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));

    open_overview(&ws, cx);
    cx.simulate_keystrokes("escape");
    to_rest(&ws, cx);
    back_on_pane(&s, &ws, cx, "esc1").unwrap();

    open_overview(&ws, cx);
    cx.simulate_keystrokes("cmd-shift-\\");
    to_rest(&ws, cx);
    back_on_pane(&s, &ws, cx, "toggle2").unwrap();

    open_overview(&ws, cx);
    cx.simulate_keystrokes("enter");
    to_rest(&ws, cx);
    let r = back_on_pane(&s, &ws, cx, "enter3");
    s.app.stop();
    r.unwrap();
}
