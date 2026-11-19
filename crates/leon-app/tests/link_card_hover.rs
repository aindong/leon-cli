//! Resting the mouse on a link with no key held shows the link card after the delay, with the
//! address, Copy and "Open in Browser ⌘Click"; the link is looked up once per cell the mouse rests
//! on, never per mouse move.
mod link_card_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use link_card_support::*;

#[gpui::test]
fn resting_on_a_link_shows_the_card_after_the_delay(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);

    rest(&ws, cx, l.web, Modifiers::none(), 300);
    assert_eq!(card(&ws, cx), None, "no card at 300 ms");
    assert_eq!(asks(&ws, cx), 0, "and no lookup yet");
    // a move inside the same cell doesn't start the wait again
    rest(&ws, cx, l.web + point(px(1.), px(0.)), Modifiers::none(), 200);
    let lines = card(&ws, cx).expect("the card shows by 500 ms");
    assert_eq!(lines.first().map(String::as_str), Some(WEB), "the address: {lines:?}");
    let items = card_items(&ws, cx);
    assert_eq!(
        items,
        vec![("copy".into(), "Copy".into(), None), ("open".into(), "Open in Browser".into(), Some("⌘Click".into())),]
    );
    assert!(at(cx, "popover").is_some(), "the card is drawn");
    assert_eq!(asks(&ws, cx), 1, "one lookup for the cell it rested on");

    rest(&ws, cx, l.web, Modifiers::none(), 2000);
    assert_eq!(asks(&ws, cx), 1, "resting longer asks nothing more");
    s.app.stop();
}

#[gpui::test]
fn moving_across_cells_asks_only_where_it_rests(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let l = links(&s, &ws, cx);
    // five moves with no rest between them, then a rest off the link
    for dx in 0..5 {
        cx.simulate_mouse_move(l.web + point(px(dx as f32 * 9.), px(0.)), None::<MouseButton>, Modifiers::none());
    }
    rest(&ws, cx, l.off, Modifiers::none(), 600);
    assert_eq!(asks(&ws, cx), 1, "one lookup, for the cell it rested on");
    assert_eq!(card(&ws, cx), None, "off the link: no card");
    s.app.stop();
}
