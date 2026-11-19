//! GPUI phase 6 (ADR R9): the find bar takes the focus while open on the pane shown and edits its
//! query; a chord it doesn't take still runs its binding (the key goes on to the workspace); its
//! pane leaving the screen, or Escape, gives the pane the focus back and typed text reaches it.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

fn open_find(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    cx.simulate_keystrokes("cmd-f");
    assert!(ws.read_with(cx, |ws, _| ws.find_query()).is_some(), "the find bar opened");
    assert_eq!(owner(ws, cx), Some("find"), "the find bar has the focus");
}

#[gpui::test]
fn every_way_the_find_bar_closes_gives_the_pane_its_keys(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));

    open_find(&ws, cx);
    cx.simulate_keystrokes("z q");
    assert_eq!(ws.read_with(cx, |ws, _| ws.find_query()).as_deref(), Some("zq"), "typing edits the query");
    cx.simulate_keystrokes("escape");
    back_on_pane(&s, &ws, cx, "esc1").unwrap();

    // ⌘D isn't the bar's: the split runs, and the new pane (shown, focused) takes the keys
    open_find(&ws, cx);
    let leaves =
        |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.active_window()).map(|w| leaves(&w).len());
    cx.simulate_keystrokes("cmd-d");
    assert!(shows(cx, |cx| leaves(cx) == Some(2)), "⌘D split the pane though the find bar had the focus");
    back_on_pane(&s, &ws, cx, "split2").unwrap();

    // its pane leaves the screen: another tab
    open_find(&ws, cx);
    cx.simulate_keystrokes("cmd-t");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.windows()).is_some_and(|(w, _)| w.len() == 2)), "a tab");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.find_query()).is_none()), "the find bar left with its pane");
    settle(&focused(&s.app, &ws, cx));
    let r = back_on_pane(&s, &ws, cx, "tab3");
    s.app.stop();
    r.unwrap();
}
