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
