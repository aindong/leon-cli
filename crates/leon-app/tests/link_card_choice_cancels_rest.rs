//! Review: a right-click right after the mouse arrives on a link, then Enter before the rest's wait
//! ends: the wait that arrival started opens no hover card once the chosen card has closed.
mod link_card_support;

use gpui::{Modifiers, MouseButton, TestAppContext};
use link_card_support::*;

#[gpui::test]
fn choosing_from_the_card_cancels_the_rest(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    cx.simulate_mouse_move(l.web, None::<MouseButton>, Modifiers::none());
    right_click(cx, l.web);
    assert!(card(&ws, cx).is_some(), "the right-click card opens");
    cx.simulate_keystrokes("enter"); // Copy, before the rest's 450 ms are up
    assert_eq!(card(&ws, cx), None, "the chosen card closed");
    tick(&ws, cx, std::time::Duration::from_millis(600));
    let reopened = card(&ws, cx);
    s.app.stop();
    assert_eq!(reopened, None, "the rest started before the right-click opened no hover card");
}
