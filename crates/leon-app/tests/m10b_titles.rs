//! M10b criterion 9: 13 pt UI text, 11 pt secondary, and tabs named by the title the program set.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::theme::{SECONDARY_TEXT, UI_TEXT};
use m10b_support::*;

#[test]
fn type_sizes() {
    assert_eq!((UI_TEXT, SECONDARY_TEXT), (13., 11.));
}

#[gpui::test]
fn a_tab_is_named_by_its_label_then_its_programs_title_then_the_program(cx: &mut TestAppContext) {
    plain_shell(); // the first title is the program's, not one a login shell's prompt sets
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let titles = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.tab_titles());
    let first = titles(cx);
    assert!(first[0].ends_with("sh"), "the program name before any title: {first:?}");
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    run_until(&term(&s.app, &id), r"printf '\033]2;fix-the-flaky-test\007'; echo t-sent; sleep 60", "t-sent");
    assert!(shows(cx, |cx| titles(cx) == vec!["fix-the-flaky-test".to_string()]), "{:?}", titles(cx));
    let b = s
        .app
        .call(
            "session.describe_block",
            serde_json::json!({
        "session_id": ws.read_with(cx, |ws, _| ws.session_id()).unwrap(), "block_id": id }),
        )
        .unwrap();
    assert_eq!(b["title"], "fix-the-flaky-test", "the server keeps the title: {b}");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let wid = window(&s.app, &ws, cx).0.id;
    s.app
        .call("session.set_window_label", serde_json::json!({ "session_id": sid, "window_id": wid, "label": "api" }))
        .unwrap();
    assert!(shows(cx, |cx| titles(cx) == vec!["api".to_string()]), "a label wins: {:?}", titles(cx));
    s.app.stop();
}
