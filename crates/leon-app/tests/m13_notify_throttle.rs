//! M13 criterion 6: what the notifier posts: entering blocked or error, done after a long run
//! while the app is in the background; nothing for what the user is looking at; one per block
//! and state a minute, four a minute in all with the rest folded into one; per-kind settings;
//! untrusted text cleaned and capped.

mod m13_support;

use std::time::{Duration, Instant};

use leon_app::attention::{Notifier, Settings, Tracker, Transition};
use leon_app::status::Indicator;
use leon_core::program_status::{Kind, State};
use m13_support::*;

fn change(block: &str, to: Indicator) -> Transition {
    Transition {
        host: 0,
        host_label: "This Mac".into(),
        session_id: "session:web".into(),
        block_id: block.into(),
        place: "This Mac › web › claude".into(),
        to,
        caption: Some("Allow edit?".into()),
        worked: None,
    }
}

const AWAY: (bool, bool) = (false, false); // the app inactive, the block not on screen
const LOOKING: (bool, bool) = (true, true);

fn offer(n: &mut Notifier, t: &Transition, now: Instant, (active, on_screen): (bool, bool)) -> bool {
    n.offer(t, now, active, on_screen).is_some()
}

#[test]
fn blocked_and_error_post_and_working_does_not() {
    let (mut n, now) = (Notifier::default(), Instant::now());
    let blocked = n.offer(&change("b1", Indicator::Blocked(Some(Kind::Permission))), now, false, false).unwrap();
    assert!(blocked.title.contains("web") && blocked.title.contains("claude"), "names the terminal: {blocked:?}");
    assert!(blocked.body.contains("Allow edit?") && blocked.body.contains("permission"), "{blocked:?}");
    assert_eq!(blocked.block_id, "b1");
    assert!(offer(&mut n, &change("b2", Indicator::Error), now, AWAY));
    assert!(!offer(&mut n, &change("b3", Indicator::Working { progress: None }), now, AWAY));
}

#[test]
fn nothing_for_the_pane_the_user_is_looking_at() {
    let (mut n, now) = (Notifier::default(), Instant::now());
    assert!(!offer(&mut n, &change("b1", Indicator::Blocked(None)), now, LOOKING));
    assert!(offer(&mut n, &change("b1", Indicator::Blocked(None)), now, (true, false)), "another tab: posted");
    assert!(offer(&mut n, &change("b2", Indicator::Error), now, (false, true)), "the app in the background: posted");
}

#[test]
fn done_posts_only_after_a_long_run_and_only_while_the_app_is_inactive() {
    let (mut n, now) = (Notifier::default(), Instant::now());
    let mut done = change("b1", Indicator::Done);
    done.worked = Some(Duration::from_secs(5));
    assert!(!offer(&mut n, &done, now, AWAY), "a short run is not worth a notice");
    done.worked = None;
    assert!(!offer(&mut n, &done, now, AWAY), "done without a run seen");
    done.worked = Some(Duration::from_secs(45));
    assert!(!offer(&mut n, &done, now, (true, false)), "the app is in front: no notice for done");
    assert!(offer(&mut n, &done, now, AWAY));
}

#[test]
fn one_per_block_and_state_a_minute() {
    let (mut n, now) = (Notifier::default(), Instant::now());
    let t = change("b1", Indicator::Blocked(None));
    assert!(offer(&mut n, &t, now, AWAY));
    assert!(!offer(&mut n, &t, now + Duration::from_secs(10), AWAY), "flapping within a minute: once");
    assert!(
        offer(&mut n, &change("b1", Indicator::Error), now + Duration::from_secs(10), AWAY),
        "another state is new"
    );
    assert!(offer(&mut n, &t, now + Duration::from_secs(61), AWAY), "a minute later it may come again");
}

#[test]
fn a_storm_is_four_a_minute_and_the_rest_fold_into_one() {
    let (mut n, now) = (Notifier::default(), Instant::now());
    let posted = (0..20).filter(|i| offer(&mut n, &change(&format!("b{i}"), Indicator::Error), now, AWAY)).count();
    assert_eq!(posted, 4);
    assert!(n.flush(now + Duration::from_secs(1)).is_none(), "the fold waits for the window");
    let fold = n.flush(now + Duration::from_secs(61)).expect("one notice for the rest");
    assert!(fold.title.contains("16"), "{fold:?}");
    assert!(n.flush(now + Duration::from_secs(62)).is_none(), "folded once");
}

#[test]
fn each_kind_can_be_turned_off() {
    let now = Instant::now();
    let mut n = Notifier { settings: Settings { blocked: false, error: true, done: true }, ..Notifier::default() };
    assert!(!offer(&mut n, &change("b1", Indicator::Blocked(None)), now, AWAY));
    assert!(offer(&mut n, &change("b2", Indicator::Error), now, AWAY));
    n.settings = Settings { blocked: true, error: false, done: false };
    assert!(!offer(&mut n, &change("b3", Indicator::Error), now, AWAY));
    let mut done = change("b4", Indicator::Done);
    done.worked = Some(Duration::from_secs(100));
    assert!(!offer(&mut n, &done, now, AWAY));
}

#[test]
fn untrusted_text_comes_out_clean_and_short() {
    let (mut n, now) = (Notifier::default(), Instant::now());
    let mut t = change("b1", Indicator::Blocked(None));
    t.place = "This Mac › web\u{202e}evil › tab\u{7}\u{1b}[31m".into();
    t.caption = Some(format!("go\u{200b}od\u{2066} {}", "x".repeat(500)));
    let notice = n.offer(&t, now, false, false).unwrap();
    for text in [&notice.title, &notice.body] {
        assert!(!text.chars().any(|c| c.is_control()), "{text:?}");
        assert!(!text.contains(['\u{202e}', '\u{200b}', '\u{2066}']), "{text:?}");
        assert!(text.chars().count() <= 120, "{} chars", text.chars().count());
    }
    assert!(notice.body.contains("good"), "{:?}", notice.body);
}

#[test]
fn the_tracker_reports_entering_a_state_and_how_long_the_run_was() {
    let mut t = Tracker::default();
    let start = Instant::now();
    let run = |records| model(vec![("web", vec![Tab(None, "make", records)])]);
    assert!(t.update(&[(0, "This Mac", &run(vec![working(10)]))], start).iter().all(|c| c.to != Indicator::Done));
    let none = t.update(&[(0, "This Mac", &run(vec![working(50)]))], start + Duration::from_secs(20));
    assert!(none.is_empty(), "progress moving is not a new state: {none:?}");
    let changes = t.update(&[(0, "This Mac", &run(vec![record(State::Done)]))], start + Duration::from_secs(40));
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].to, Indicator::Done);
    assert_eq!(changes[0].worked, Some(Duration::from_secs(40)), "working since the first time it was seen");
    assert_eq!(changes[0].place, "This Mac › web › make");
    assert!(t.update(&[(0, "This Mac", &run(vec![record(State::Done)]))], start + Duration::from_secs(41)).is_empty());
}
