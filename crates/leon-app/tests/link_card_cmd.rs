//! With ⌘ held there is no link card: ⌘-hover underlines and ⌘-click opens at once, as before.
mod link_card_support;

use gpui::{Modifiers, TestAppContext};
use link_card_support::*;

#[gpui::test]
fn holding_cmd_shows_no_card(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, cmd(), 600);
    assert_eq!(card(&ws, cx), None, "⌘ held: no card");
    assert_eq!(asks(&ws, cx), 0, "and no card lookup");
    assert_eq!(ws.read_with(cx, |ws, _| ws.hovered_link()).as_deref(), Some(WEB), "the underline still shows");
    s.app.stop();
}

#[gpui::test]
fn cmd_pressed_during_the_rest_cancels_the_card(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    rest(&ws, cx, l.web, Modifiers::none(), 200);
    cx.simulate_modifiers_change(cmd());
    tick(&ws, cx, std::time::Duration::from_millis(400));
    assert_eq!(card(&ws, cx), None, "⌘ pressed at 200 ms: no card at 600 ms");

    press(cx, l.web, cmd(), 1);
    release(cx, l.web, cmd(), 1);
    cx.run_until_parked();
    assert_eq!(*l.opened.borrow(), vec![WEB.to_string()], "⌘-click opens at once");
    assert!(!cx.has_pending_prompt());
    assert_eq!(card(&ws, cx), None, "and shows no card");
    s.app.stop();
}
