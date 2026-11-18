//! The × on hover: on a sidebar tab row and session label, a top-style session row and a titlebar
//! tab, each closing by the same rule as the palette (a running program asks first).
mod close_support;

use close_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn the_sidebar_rows_close_their_tab_and_session(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (sid, _) = busy_tab(&s, &ws, cx);
    hover_click(cx, "row-0-1", "row-close-0-1");
    assert!(cx.has_pending_prompt(), "the busy tab asks");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(tabs(&s, &sid), 2, "Cancel keeps it");
    let before = window(&s.app, &ws, cx).0.id;
    hover_click(cx, "row-0-0", "row-close-0-0");
    assert!(!cx.has_pending_prompt(), "a shell's tab doesn't ask");
    assert_eq!(window(&s.app, &ws, cx).0.id, before, "the × selected nothing first");
    assert!(shows(cx, |_| tabs(&s, &sid) == 1), "the shell's tab closed");
    hover_click(cx, "session-0", "session-close-0");
    assert!(cx.has_pending_prompt(), "the session still runs sleep: it asks");
    cx.simulate_prompt_answer("Close Session");
    cx.run_until_parked();
    let gone = shows(cx, |_| tabs(&s, &sid) == 0);
    s.app.stop();
    assert!(gone, "the session closed");
}

#[gpui::test]
fn the_titlebar_tab_and_top_style_session_row_ask_too(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "client.tab_style.toggle");
    let (sid, _) = busy_tab(&s, &ws, cx);
    hover_click(cx, "tab-1", "close-1");
    assert!(cx.has_pending_prompt(), "the titlebar's × asks about sleep");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert_eq!(tabs(&s, &sid), 2, "Cancel keeps it");
    hover_click(cx, "session-0", "session-close-0");
    assert!(cx.has_pending_prompt(), "the session row's × asks");
    cx.simulate_prompt_answer("Close Session");
    cx.run_until_parked();
    let gone = shows(cx, |_| tabs(&s, &sid) == 0);
    s.app.stop();
    assert!(gone, "the session closed");
}
