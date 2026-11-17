//! Review proof (correctness): a selection drag released outside the pane still finishes, so
//! copy-on-select copies.
mod m11_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use m11_support::*;

#[gpui::test]
fn copy_on_select_when_released_outside_the_pane(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"copy_on_select\", true)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let id = tab_running(&s, &ws, cx, "stty -echo; seq 1 100; sleep 100", "100");
    let r = row_of(&ws, cx, &id, "99");
    let (from, to) = (cell(&ws, cx, &id, r, 0), cell(&ws, cx, &id, r, 1));
    press(cx, from, Modifiers::none(), 1);
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    release(cx, point(px(1.), px(1.)), Modifiers::none(), 1); // titlebar/sidebar, not the pane
    cx.run_until_parked();
    let got = clipboard(cx);
    s.app.stop();
    assert_eq!(got.as_deref(), Some("99"));
}
