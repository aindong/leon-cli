//! Close Tab (palette, ⌥⌘W): a tab running only its shells closes at once; one running another
//! program asks first, naming it, and Cancel keeps it.
mod close_support;

use close_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_tab_of_shells_closes_at_once(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    act(&ws, cx, "client.tab.new");
    assert!(shows(cx, |_| tabs(&s, &sid) == 2));
    shells_only(&ws, cx, &sid);
    act(&ws, cx, "client.tab.close");
    assert!(!cx.has_pending_prompt(), "no question");
    let closed = shows(cx, |_| tabs(&s, &sid) == 1);
    s.app.stop();
    assert!(closed, "the tab closed");
}

#[gpui::test]
fn a_tab_running_a_program_asks_first(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (sid, _) = busy_tab(&s, &ws, cx);
    cx.simulate_keystrokes("cmd-alt-w");
    let asked = ("Close this tab?".to_string(), "sleep is running in it and stops.".to_string());
    assert_eq!(cx.pending_prompt(), Some(asked), "⌥⌘W asks, naming the program");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(tabs(&s, &sid), 2, "Cancel keeps it");
    act(&ws, cx, "client.tab.close");
    assert!(cx.has_pending_prompt(), "the palette's action asks too");
    cx.simulate_prompt_answer("Close Tab");
    cx.run_until_parked();
    let closed = shows(cx, |_| tabs(&s, &sid) == 1);
    s.app.stop();
    assert!(closed, "Close Tab closed it");
}
