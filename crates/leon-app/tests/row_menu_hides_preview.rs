//! Review (screenshot): a row's menu and a tab's hover preview never show together; opening the
//! menu hides the preview, and resting on a tab while it is open shows none.
mod close_support;

use close_support::*;
use gpui::{Modifiers, TestAppContext};
use std::time::Duration;

#[gpui::test]
fn the_menu_hides_the_preview(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab.new");
    let row = center(at(cx, "row-0-0").expect("the first tab's row"));
    cx.simulate_mouse_move(row, None, Modifiers::none());
    tick(&ws, cx, Duration::from_millis(600));
    assert!(ws.read_with(cx, |ws, _| ws.hover_preview()).is_some(), "setup: the preview shows");
    right_click(cx, row);
    assert!(ws.read_with(cx, |ws, _| ws.row_menu()).is_some(), "the menu opened");
    assert_eq!(ws.read_with(cx, |ws, _| ws.hover_preview()), None, "and the preview went");
    let other = center(at(cx, "row-0-1").expect("the second tab's row"));
    cx.simulate_mouse_move(other, None, Modifiers::none());
    tick(&ws, cx, Duration::from_millis(600));
    let shown = ws.read_with(cx, |ws, _| ws.hover_preview());
    s.app.stop();
    assert_eq!(shown, None, "no preview while the menu is open");
}
