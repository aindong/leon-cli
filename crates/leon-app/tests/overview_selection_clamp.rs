//! Review proof (correctness): a tab closing under the overview leaves the selection on a card, so
//! the arrows and ↩ still work.

mod m10b_support;

use gpui::TestAppContext;
use m10b_support::*;
use serde_json::json;

#[gpui::test]
fn the_selection_stays_on_a_card_when_a_tab_closes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    act(&ws, cx, "client.tab.new");
    act(&ws, cx, "client.tab.new");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    act(&ws, cx, "client.overview.toggle");
    let cards = ws.read_with(cx, |ws, _| ws.overview_cards());
    assert_eq!(cards.len(), 3);
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_selected()), 2, "the current (last) tab is selected");
    s.app.call("session.close_window", json!({ "session_id": sid, "window_id": cards[0].window_id })).unwrap();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.overview_cards().len()) == 2), "the closed tab's card goes");
    cx.simulate_keystrokes("left");
    let sel = ws.read_with(cx, |ws, _| ws.overview_selected());
    assert!(sel < 1, "the selection is off the end and the arrows can't move it: {sel}");
    s.app.stop();
}

#[gpui::test]
fn the_selection_stays_on_its_tab_when_one_before_it_closes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    act(&ws, cx, "client.tab.new");
    act(&ws, cx, "client.tab.new");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    act(&ws, cx, "client.overview.toggle");
    let cards = ws.read_with(cx, |ws, _| ws.overview_cards());
    cx.simulate_keystrokes("left"); // B, in [A, B, C]
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_selected()), 1);
    s.app.call("session.close_window", json!({ "session_id": sid, "window_id": cards[0].window_id })).unwrap();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.overview_cards().len()) == 2), "A's card goes");
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_selected()), 0, "still B, now first");
    cx.simulate_keystrokes("enter");
    let went = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.active_window()).is_some_and(|w| w.id == cards[1].window_id));
    assert!(went, "↩ goes to B, not to the card that took its place");
    s.app.stop();
}

#[gpui::test]
fn opening_scrolls_the_current_tabs_card_into_view(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    for _ in 0..40 {
        s.app.call("session.new_window", json!({ "session_id": sid })).unwrap();
    }
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.windows()).is_some_and(|(w, _)| w.len() == 41)));
    let current = ws.read_with(cx, |ws, _| ws.active_window()).unwrap().id;
    act(&ws, cx, "client.overview.toggle");
    let card = at(cx, &format!("card-{current}")).expect("the current tab's card is drawn");
    assert!(f32::from(card.origin.y + card.size.height) <= 900., "the last tab's card is in view: {card:?}");
    s.app.stop();
}
