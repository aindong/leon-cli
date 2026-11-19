//! A right-click right after the mouse arrives on a link: the wait that arrival started doesn't
//! swap the keyboard card for a hover card when it ends.
mod link_card_support;

use gpui::{Modifiers, MouseButton, TestAppContext};
use link_card_support::*;

#[gpui::test]
fn the_rest_wait_leaves_a_right_click_card_alone(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    cx.simulate_mouse_move(l.web, None::<MouseButton>, Modifiers::none());
    right_click(cx, l.web);
    let first = ws.read_with(cx, |ws, _| ws.link_card()).expect("the card opens");
    tick(&ws, cx, std::time::Duration::from_millis(600));
    let now = ws.read_with(cx, |ws, _| ws.link_card());
    let highlighted = now.as_ref().and_then(|p| ws.read_with(cx, |_, cx| p.read(cx).highlighted()));
    s.app.stop();
    assert_eq!(now.map(|p| p.entity_id()), Some(first.entity_id()), "the same card");
    assert_eq!(highlighted, Some(0), "still the keyboard's");
}
