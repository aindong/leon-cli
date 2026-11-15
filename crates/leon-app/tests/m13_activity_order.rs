//! M13 criterion 3: Activity rows come blocked, error, done, working; the longest-waiting first
//! inside a group; each names its host › session › tab, and a blocked row its kind.

mod m13_support;

use std::time::{Duration, Instant};

use leon_app::attention::{order, state_text, Row, Tracker};
use leon_app::status::Indicator;
use leon_core::program_status::{Kind, State};
use m13_support::*;

fn row(place: &str, status: Indicator, waited: u64, now: Instant) -> Row {
    Row {
        host: 0,
        session_id: "session:x".into(),
        block_id: format!("block:{place}"),
        place: place.into(),
        title: place.rsplit(" › ").next().unwrap().into(),
        status,
        caption: None,
        since: now - Duration::from_secs(waited),
    }
}

#[test]
fn blocked_then_error_then_done_then_working_longest_waiting_first() {
    let now = Instant::now();
    let mut rows = vec![
        row("This Mac › web › build", Indicator::Working { progress: Some(40) }, 50, now),
        row("This Mac › web › tests", Indicator::Done, 5, now),
        row("devbox › api › deploy", Indicator::Error, 1, now),
        row("This Mac › web › claude", Indicator::Blocked(Some(Kind::Question)), 3, now),
        row("This Mac › docs › codex", Indicator::Blocked(Some(Kind::Permission)), 30, now),
        row("This Mac › web › lint", Indicator::Done, 60, now),
    ];
    order(&mut rows);
    let places: Vec<&str> = rows.iter().map(|r| r.place.as_str()).collect();
    assert_eq!(
        places,
        vec![
            "This Mac › docs › codex",
            "This Mac › web › claude",
            "devbox › api › deploy",
            "This Mac › web › lint",
            "This Mac › web › tests",
            "This Mac › web › build",
        ]
    );
}

#[test]
fn a_rows_state_says_the_blocked_kind_and_the_progress() {
    assert_eq!(state_text(Indicator::Blocked(Some(Kind::Question))), "blocked: question");
    assert_eq!(state_text(Indicator::Blocked(Some(Kind::Permission))), "blocked: permission");
    assert_eq!(state_text(Indicator::Blocked(Some(Kind::Auth))), "blocked: auth");
    assert_eq!(state_text(Indicator::Blocked(None)), "blocked");
    assert_eq!(state_text(Indicator::Error), "error");
    assert_eq!(state_text(Indicator::Done), "done");
    assert_eq!(state_text(Indicator::Working { progress: Some(40) }), "working 40%");
    assert_eq!(state_text(Indicator::Working { progress: None }), "working");
}

#[test]
fn the_tracker_lists_every_hosts_blocks_that_need_you_in_order() {
    let here = model(vec![
        (
            "web",
            vec![
                Tab(None, "build", vec![working(40)]),
                Tab(None, "zsh", vec![]),
                Tab(None, "tests", vec![record(State::Done)]),
            ],
        ),
        ("docs", vec![Tab(None, "idle", vec![record(State::Idle)])]),
    ]);
    let devbox = model(vec![(
        "api",
        vec![Tab(None, "claude", vec![blocked(Kind::Question)]), Tab(None, "deploy", vec![record(State::Error)])],
    )]);
    let mut t = Tracker::default();
    let start = Instant::now();
    t.update(&[(0, "This Mac", &here), (1, "devbox", &devbox)], start);
    let rows = t.rows();
    let places: Vec<&str> = rows.iter().map(|r| r.place.as_str()).collect();
    assert_eq!(
        places,
        vec!["devbox › api › claude", "devbox › api › deploy", "This Mac › web › tests", "This Mac › web › build"]
    );
    assert_eq!(rows[0].host, 1);
    assert_eq!(state_text(rows[0].status), "blocked: question");
    assert_eq!(state_text(rows[3].status), "working 40%");
    assert_eq!(rows[0].since, start, "waiting since it was first seen blocked");
    assert_eq!(t.blocked(), 1);
}
