//! M9 criterion 1: OSC 7501 records to the indicator a tab, a sidebar row and a pane corner show.

use leon_app::status::{aggregate, caption_of, glyph, indicator_of, Indicator};
use leon_core::program_status::{parse, Change, Kind, Message, Reason, Record, State, Store};

fn rec(state: State) -> Record {
    Record { state, kind: None, progress: None, app: None, title: None, msg: None }
}

fn one(r: Record) -> Vec<(Option<String>, Record)> {
    vec![(None, r)]
}

/// A terminal's indicator and caption from its records, least recently reported first.
fn indicator(records: &[(Option<String>, Record)]) -> Option<Indicator> {
    indicator_of(&records.iter().rev().map(|(_, r)| r).collect::<Vec<_>>())
}

fn caption(records: &[(Option<String>, Record)]) -> Option<String> {
    caption_of(&records.iter().rev().map(|(_, r)| r).collect::<Vec<_>>())
}

#[test]
fn each_state_maps_to_its_indicator() {
    assert_eq!(indicator(&[]), None);
    assert_eq!(indicator(&one(rec(State::Idle))), None, "idle shows nothing");
    assert_eq!(indicator(&one(rec(State::Working))), Some(Indicator::Working { progress: None }));
    let w = Record { progress: Some(40), ..rec(State::Working) };
    assert_eq!(indicator(&one(w)), Some(Indicator::Working { progress: Some(40) }));
    for kind in [Kind::Permission, Kind::Question, Kind::Auth] {
        let b = Record { kind: Some(kind), ..rec(State::Blocked) };
        assert_eq!(indicator(&one(b)), Some(Indicator::Blocked(Some(kind))));
    }
    assert_eq!(indicator(&one(rec(State::Done))), Some(Indicator::Done));
    assert_eq!(indicator(&one(rec(State::Error))), Some(Indicator::Error));
}

#[test]
fn the_highest_priority_wins_within_a_pane_and_across_panes() {
    let recs =
        vec![(None, rec(State::Done)), (Some("a".into()), rec(State::Working)), (Some("b".into()), rec(State::Error))];
    assert_eq!(indicator(&recs), Some(Indicator::Error), "error beats working and done");
    let mut with_blocked = recs.clone();
    with_blocked.push((Some("c".into()), Record { kind: Some(Kind::Question), ..rec(State::Blocked) }));
    assert_eq!(indicator(&with_blocked), Some(Indicator::Blocked(Some(Kind::Question))), "blocked beats all");

    // a session's row: the highest among its panes
    let panes = [None, Some(Indicator::Done), Some(Indicator::Working { progress: Some(5) }), None];
    assert_eq!(aggregate(panes), Some(Indicator::Working { progress: Some(5) }));
    assert_eq!(aggregate([Some(Indicator::Done), Some(Indicator::Error)]), Some(Indicator::Error));
    assert_eq!(aggregate([None, None]), None);
    assert_eq!(aggregate([Some(Indicator::Blocked(None)), Some(Indicator::Error)]), Some(Indicator::Blocked(None)));
}

#[test]
fn glyphs_differ_per_state_and_the_spinner_turns() {
    assert_eq!(glyph(Indicator::Done, 0), "✓");
    assert_eq!(glyph(Indicator::Error, 0), "✗");
    let spin0 = glyph(Indicator::Working { progress: None }, 0);
    let spin1 = glyph(Indicator::Working { progress: None }, 1);
    assert_ne!(spin0, spin1, "the spinner moves frame to frame");
    assert!(glyph(Indicator::Working { progress: Some(40) }, 0).contains("40%"));
    let kinds = [Kind::Permission, Kind::Question, Kind::Auth].map(|k| glyph(Indicator::Blocked(Some(k)), 0));
    assert!(kinds[0] != kinds[1] && kinds[1] != kinds[2] && kinds[0] != kinds[2], "{kinds:?}");
}

#[test]
fn the_caption_is_isolated_and_short() {
    let r = Record { title: Some("Claude".into()), msg: Some("needs you".into()), ..rec(State::Blocked) };
    let c = caption(&one(r)).unwrap();
    assert!(c.starts_with('\u{2068}') && c.ends_with('\u{2069}'), "bidi-isolated: {c:?}");
    assert!(c.contains("Claude") && c.contains("needs you"), "{c:?}");
    let long = Record { msg: Some("x".repeat(2000)), ..rec(State::Working) };
    assert!(caption(&one(long)).unwrap().chars().count() <= 122, "cut to 120 chars plus the isolates");
    assert_eq!(caption(&one(rec(State::Working))), None, "no title or msg, no caption");
    assert_eq!(caption(&one(Record { title: Some("t".into()), ..rec(State::Idle) })), None, "idle: nothing shown");
}

#[test]
fn acknowledging_clears_only_done_and_error() {
    let mut s = Store::default();
    for (id, state) in [(None, "done"), (Some("a"), "error"), (Some("b"), "working"), (Some("c"), "blocked")] {
        let id = id.map(|i| format!(":id={i}")).unwrap_or_default();
        let Ok(m @ Message::Set { .. }) = parse(format!("state={state}{id}").as_bytes()) else { panic!() };
        s.apply(m);
    }
    let gone = s.acknowledge();
    assert_eq!(gone, vec![Change::Removed { ids: vec![None, Some("a".into())], reason: Reason::Seen }]);
    let left: Vec<_> = s.records().into_iter().map(|(id, r)| (id, r.state)).collect();
    assert_eq!(left, vec![(Some("b".into()), State::Working), (Some("c".into()), State::Blocked)]);
    assert!(s.acknowledge().is_empty(), "nothing left to clear: no event");
}
