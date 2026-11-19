//! Review proof (cross-model): the toolbar's palette button, pressed while the palette is open,
//! closes it (the press outside the palette closing it must not be undone by the button).
mod focus_support;

use focus_support::*;
use gpui::{Modifiers, TestAppContext};

#[gpui::test]
fn the_palette_button_closes_an_open_palette(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the palette opened");
    let button = center(at(cx, "toolbar-palette").expect("the toolbar's palette button"));
    cx.simulate_click(button, Modifiers::none());
    let open = ws.read_with(cx, |ws, _| ws.palette_open());
    let r = back_on_pane(&s, &ws, cx, "button1");
    s.app.stop();
    assert!(!open, "the palette button left the palette open");
    r.unwrap();
}
