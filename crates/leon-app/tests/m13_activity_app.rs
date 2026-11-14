//! M13 criterion 4: ⌘⇧A shows the Activity list; a row's click jumps to its pane, and arriving
//! there clears its done (spec §5 lifetime), so the row goes.

mod m10b_support;

use gpui::{Modifiers, TestAppContext};
use leon_app::status::Indicator;
use m10b_support::*;
use serde_json::json;

const DONE: &str = r"printf '\033]7501;state=done:title=dGVzdHM=\007'; echo d-sent";
const ERROR: &str = r"printf '\033]7501;state=error\007'; echo e-sent";

#[gpui::test]
fn activity_lists_what_needs_you_and_a_click_jumps_and_clears_done(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let here = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let other = s.app.call("session.create", json!({ "label": "ci" })).unwrap();
    let (other_sid, other_block) =
        (other["session_id"].as_str().unwrap().to_string(), other["block_id"].as_str().unwrap().to_string());
    run_until(&term(&s.app, &other_block), DONE, "d-sent");
    run_until(&term(&s.app, &here), ERROR, "e-sent");

    assert!(!ws.read_with(cx, |ws, _| ws.activity_visible()));
    cx.simulate_keystrokes("cmd-shift-a");
    assert!(ws.read_with(cx, |ws, _| ws.activity_visible()), "cmd+shift+a shows Activity");
    let listed = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.activity_rows().len()) == 2);
    let rows = ws.read_with(cx, |ws, _| ws.activity_rows());
    assert!(listed, "both blocks are listed: {:?}", rows.iter().map(|r| &r.place).collect::<Vec<_>>());
    assert_eq!(rows[0].status, Indicator::Error, "error before done");
    assert_eq!(rows[1].block_id, other_block);
    assert!(rows[1].place.starts_with("This Mac › ci › "), "{}", rows[1].place);
    assert!(rows[1].caption.as_deref().is_some_and(|c| c.contains("tests")), "{:?}", rows[1].caption);

    let row = at(cx, "activity-row-1").expect("the done row is drawn");
    cx.simulate_click(center(row), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(other_sid), "the click selects its session");
    assert_eq!(ws.read_with(cx, |ws, _| ws.target_block()), Some(other_block.clone()), "and its pane");
    let gone = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.activity_rows().iter().all(|r| r.block_id != other_block)));
    assert!(gone, "arriving at the pane cleared its done, so its row went");
    assert!(term(&s.app, &other_block).program_status().is_empty());

    cx.simulate_keystrokes("cmd-shift-a");
    assert!(!ws.read_with(cx, |ws, _| ws.activity_visible()), "cmd+shift+a hides it again");
    s.app.stop();
}

#[gpui::test]
fn the_sidebar_has_an_activity_button(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let button = at(cx, "toolbar-activity").expect("an Activity button in the toolbar");
    cx.simulate_click(center(button), Modifiers::none());
    assert!(ws.read_with(cx, |ws, _| ws.activity_visible()));
    assert!(drawn(cx, "activity"), "the Activity list is drawn");
    s.app.stop();
}
