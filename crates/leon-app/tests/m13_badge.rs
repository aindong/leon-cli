//! M13 criterion 5: the toolbar's Activity button shows how many blocks are blocked, only when
//! some are; the Dock badge says the same (nothing at 0).

mod m10b_support;

use gpui::TestAppContext;
use leon_app::attention::badge;
use m10b_support::*;
use serde_json::json;

const BLOCKED: &str = r"printf '\033]7501;state=blocked:kind=permission\007'; echo b-sent; sleep 60";
const DONE: &str = r"printf '\033]7501;state=done\007'; echo d-sent";

#[test]
fn the_badge_text_is_the_count_and_nothing_at_zero() {
    assert_eq!(badge(0), None);
    assert_eq!(badge(1).as_deref(), Some("1"));
    assert_eq!(badge(12).as_deref(), Some("12"));
    assert_eq!(badge(1000).as_deref(), Some("99+"));
}

#[gpui::test]
fn the_toolbar_and_dock_count_blocked_blocks_only(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let other = s.app.call("session.create", json!({})).unwrap();
    let b = other["block_id"].as_str().unwrap().to_string();

    run_until(&term(&s.app, &a), DONE, "d-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.activity_rows().len()) == 1));
    assert!(!drawn(cx, "toolbar-badge"), "done alone draws no badge");
    assert_eq!(ws.read_with(cx, |ws, _| ws.dock_badge()), None);

    run_until(&term(&s.app, &a), BLOCKED, "b-sent");
    run_until(&term(&s.app, &b), BLOCKED, "b-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.dock_badge()).as_deref() == Some("2")), "the Dock says 2");
    assert!(drawn(cx, "toolbar-badge"), "the Activity button shows a count");
    assert_eq!(ws.read_with(cx, |ws, _| ws.activity_badge()).as_deref(), Some("2"));
    s.app.stop();
}
