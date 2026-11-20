//! GPUI phase 6 (ADR R9): the palette takes the window's focus while open and edits its own
//! query; every way it closes gives the focus back to the pane, and typed text reaches it.
mod focus_support;

use focus_support::*;
use gpui::{Modifiers, TestAppContext};

fn open_palette(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the palette opened");
    assert_eq!(owner(ws, cx), Some("palette"), "the palette has the focus");
}

#[gpui::test]
fn every_way_the_palette_closes_gives_the_pane_its_keys(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    assert_eq!(owner(&ws, cx), Some("pane"), "the pane has the focus at first");

    open_palette(&ws, cx);
    cx.simulate_keystrokes("s p l");
    assert_eq!(ws.read_with(cx, |ws, _| ws.palette_query()), "spl", "typing edits the query");
    cx.simulate_keystrokes("escape");
    back_on_pane(&s, &ws, cx, "esc1").unwrap();

    open_palette(&ws, cx);
    cx.simulate_keystrokes("cmd-shift-p");
    back_on_pane(&s, &ws, cx, "toggle2").unwrap();

    open_palette(&ws, cx);
    cx.simulate_keystrokes("t o g g l e space s i d e b a r enter");
    back_on_pane(&s, &ws, cx, "enter3").unwrap();

    open_palette(&ws, cx);
    cx.simulate_keystrokes("s p l i t space r i g h t");
    let row = center(at(cx, "palette-row-0").expect("a palette row"));
    cx.simulate_click(row, Modifiers::none());
    back_on_pane(&s, &ws, cx, "click4").unwrap();

    // its Rename Tab… prompt: Enter renames and closes it
    act(&ws, cx, "client.tab.rename");
    assert_eq!(owner(&ws, cx), Some("palette"), "the prompt has the focus");
    cx.simulate_keystrokes("l o g s enter");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == ["logs"]), "renamed");
    back_on_pane(&s, &ws, cx, "ask5").unwrap();

    // the window going inactive and back leaves it open, with the focus
    open_palette(&ws, cx);
    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "still open after the window came back");
    assert_eq!(owner(&ws, cx), Some("palette"), "and it has the focus");

    let pane = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let p = center(at(cx, &format!("terminal-{pane}")).expect("the pane"));
    cx.simulate_click(p, Modifiers::none());
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "a click on a pane closes it");
    let r = back_on_pane(&s, &ws, cx, "pane6");
    s.app.stop();
    r.unwrap();
}
