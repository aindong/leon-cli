//! Close Session (palette, ⌥⇧⌘W): the same rule as a tab, for every tab of the session.
mod close_support;

use close_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_session_of_shells_closes_at_once(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "session.new");
    assert!(shows(cx, |_| sessions(&s) == 2));
    act(&ws, cx, "session.close");
    assert!(!cx.has_pending_prompt(), "no question");
    let closed = shows(cx, |_| sessions(&s) == 1);
    s.app.stop();
    assert!(closed, "the session closed");
}

#[gpui::test]
fn a_session_running_a_program_asks_first(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (sid, _) = busy_tab(&s, &ws, cx);
    act(&ws, cx, "client.tab.previous"); // the session's other tab, a shell: the session still runs sleep
    cx.simulate_keystrokes("cmd-alt-shift-w");
    assert!(cx.has_pending_prompt(), "⌥⇧⌘W asks");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(tabs(&s, &sid), 2, "Cancel keeps it");
    act(&ws, cx, "session.close");
    cx.simulate_prompt_answer("Close Session");
    cx.run_until_parked();
    let closed = shows(cx, |_| tabs(&s, &sid) == 0);
    s.app.stop();
    assert!(closed, "Close Session destroyed it");
}
