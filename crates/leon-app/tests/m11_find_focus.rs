//! Review proof (skeptic): the find bar takes typing only in its own pane: after focus moves to
//! another tab, keys reach that tab's program, not the hidden bar's query.
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

#[gpui::test]
fn typing_in_another_tab_does_not_go_to_a_find_bar_left_open(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = tab_running(&s, &ws, cx, "echo a-ready; sleep 100", "a-ready");
    cx.simulate_keystrokes("cmd-f");
    assert!(drawn(cx, &format!("find-bar-{a}")), "the bar opened in tab A");

    // a second tab takes the focus; the find bar stays behind in A
    let b = tab_running(&s, &ws, cx, "stty -echo; echo b-ready; read line; echo got:$line; sleep 100", "b-ready");
    assert_ne!(a, b);
    cx.simulate_keystrokes("h i enter");

    let reached = shows(cx, |cx| view_lines(&ws, cx, &b).iter().any(|l| l == "got:hi"));
    let query = ws.read_with(cx, |ws, _| ws.find_status());
    assert!(reached, "tab B's program never got the typing; find status in A is {query:?}");
    s.app.stop();
}
