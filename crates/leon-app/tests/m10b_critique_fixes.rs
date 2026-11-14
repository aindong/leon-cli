//! Fixes from the M10b impeccable critique: a pane header says its state in a word when the
//! program sent no caption, and with no session the sidebar steps aside so the empty state centres.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::status::{word, Indicator};
use m10b_support::*;

#[test]
fn every_state_has_a_word() {
    assert_eq!(word(Indicator::Working { progress: None }), "working");
    assert_eq!(word(Indicator::Blocked(None)), "needs you");
    assert_eq!(word(Indicator::Done), "done");
    assert_eq!(word(Indicator::Error), "error");
}

#[gpui::test]
fn a_header_without_a_caption_says_the_state(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    run_until(&focused(&s.app, &ws, cx), r"printf '\033]7501;state=working\007'; echo w-sent; sleep 60", "w-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.header_caption(&id)).as_deref() == Some("working")));
    s.app.stop();
}

#[gpui::test]
fn no_session_no_sidebar(cx: &mut TestAppContext) {
    let s = setup();
    let (_ws, cx) = open(cx, &s.app);
    assert!(drawn(cx, "sidebar"));
    cx.simulate_keystrokes("cmd-w");
    assert!(!drawn(cx, "sidebar") && drawn(cx, "empty-state"), "the empty state has the window to itself");
    assert!(drawn(cx, "toolbar-sidebar"), "the toolbar moves to the titlebar");
    cx.simulate_keystrokes("cmd-n");
    assert!(drawn(cx, "sidebar"), "and the sidebar comes back with a session");
    s.app.stop();
}
