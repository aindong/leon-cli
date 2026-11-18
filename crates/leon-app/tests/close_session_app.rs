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
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    shells_only(&ws, cx, &sid);
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
    let settled = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.running_programs(&sid, None)) == ["sleep"]);
    assert!(settled, "the first tab's shell settled");
    cx.simulate_keystrokes("cmd-alt-shift-w");
    let asked = ("Close this session?".to_string(), "sleep is running in it and stops.".to_string());
    assert_eq!(cx.pending_prompt(), Some(asked), "⌥⇧⌘W asks, naming the program");
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
