//! Idle stays idle: once the card shows, a mouse resting on the link draws no more frames and
//! makes no more lookups.
mod link_card_support;

use std::time::Duration;

use gpui::{Modifiers, TestAppContext};
use link_card_support::*;

#[gpui::test]
fn a_resting_mouse_draws_nothing_once_the_card_shows(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 500);
    assert!(card(&ws, cx).is_some(), "the card shows");
    tick(&ws, cx, Duration::from_millis(500));
    let before = ws.read_with(cx, |ws, _| ws.renders());
    for _ in 0..10 {
        tick(&ws, cx, Duration::from_millis(500));
    }
    let (after, asked) = (ws.read_with(cx, |ws, _| ws.renders()), asks(&ws, cx));
    s.app.stop();
    assert_eq!(after, before, "no frames while the mouse rests");
    assert_eq!(asked, 1, "and no more lookups");
}
