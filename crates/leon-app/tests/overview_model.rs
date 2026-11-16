//! Overview criteria 3 and 14 (.agent/plans/2026-11-17-overview-grid.md): the cards in groups, the
//! arrow keys over the grid, and a host that can't be reached greyed with the reason.

use std::collections::HashMap;

use leon_app::overview::{self, Machine, Scope, Step};
use leon_app::remote::{Block, Model, Session};
use leon_core::layout::Node;
use leon_core::program_status::{Record, State};
use leon_core::Window;

fn window(id: &str, block: &str) -> Window {
    Window {
        id: id.into(),
        label: Some(format!("tab {id}")),
        layout: Some(Node::Block { block_id: block.into() }),
        focus: Some(block.into()),
        zoomed: None,
    }
}

fn block(id: &str, window: &str, status: Option<State>) -> Block {
    let record = |state| Record { state, kind: None, progress: None, app: None, title: None, msg: None };
    Block {
        id: id.into(),
        window_id: window.into(),
        label: None,
        command: vec!["/bin/sh".into()],
        title: None,
        cwd: None,
        status: status.map(record).into_iter().collect(),
    }
}

/// A session `id` with `tabs` tabs, each one block; the first tab's block is blocked.
fn session(id: &str, label: &str, tabs: usize) -> Session {
    let windows: Vec<Window> = (0..tabs).map(|i| window(&format!("{id}-w{i}"), &format!("{id}-b{i}"))).collect();
    let blocks = (0..tabs)
        .map(|i| block(&format!("{id}-b{i}"), &format!("{id}-w{i}"), (i == 0).then_some(State::Blocked)))
        .collect();
    Session {
        id: id.into(),
        label: Some(label.into()),
        revision: 1,
        active: 0,
        windows,
        blocks,
        overlays: HashMap::new(),
        tiled_focus: HashMap::new(),
    }
}

fn model(sessions: Vec<Session>) -> Model {
    Model { sessions, ..Default::default() }
}

#[test]
fn every_tab_is_a_card_under_its_session_and_host() {
    let here = model(vec![session("a", "web", 3), session("b", "ci", 2)]);
    let devbox = model(vec![session("c", "build", 1)]);
    let machines = [
        Machine { host: 0, label: "This Mac", model: Some(&here), note: None },
        Machine { host: 1, label: "devbox", model: Some(&devbox), note: None },
    ];
    let groups = overview::groups(&machines, &Scope::All, true);
    let names: Vec<_> = groups.iter().map(|g| (g.host_label.clone(), g.session.clone(), g.cards.len())).collect();
    assert_eq!(
        names,
        vec![
            (Some("This Mac".into()), Some("web".into()), 3),
            (None, Some("ci".into()), 2),
            (Some("devbox".into()), Some("build".into()), 1),
        ]
    );
    let first = &groups[0].cards[0];
    assert_eq!((first.title.as_str(), first.window_id.as_str()), ("tab a-w0", "a-w0"));
    assert_eq!(first.status, Some(leon_app::status::Indicator::Blocked(None)), "the tab's status, as the sidebar");
    assert!(groups.iter().flat_map(|g| &g.cards).all(|c| !c.greyed));

    // without hosts there are no host labels; one session shows only its tabs
    let only = overview::groups(&machines[..1], &Scope::Session { host: 0, session_id: "b".into() }, false);
    assert_eq!(only.len(), 1);
    assert_eq!((only[0].host_label.clone(), only[0].cards.len()), (None, 2));
}

#[test]
fn a_host_that_is_not_connected_is_greyed_with_the_reason() {
    let here = model(vec![session("a", "web", 1)]);
    let stale = model(vec![session("c", "build", 2)]);
    let machines = [
        Machine { host: 0, label: "This Mac", model: Some(&here), note: None },
        Machine { host: 1, label: "devbox", model: Some(&stale), note: Some("reconnecting (try 2)".into()) },
        Machine { host: 2, label: "nas", model: None, note: Some("unreachable: no route to host".into()) },
    ];
    let groups = overview::groups(&machines, &Scope::All, true);
    let devbox: Vec<_> = groups.iter().filter(|g| g.host == 1).collect();
    assert_eq!(devbox[0].note.as_deref(), Some("reconnecting (try 2)"), "the reason under the host's label");
    assert!(devbox.iter().flat_map(|g| &g.cards).all(|c| c.greyed), "its last-known cards are greyed");
    assert_eq!(devbox.iter().map(|g| g.cards.len()).sum::<usize>(), 2);
    let nas: Vec<_> = groups.iter().filter(|g| g.host == 2).collect();
    assert_eq!(nas.len(), 1, "a host never reached still has its label");
    assert_eq!(nas[0].note.as_deref(), Some("unreachable: no route to host"));
    assert!(nas[0].cards.is_empty() && nas[0].session.is_none());
}

#[test]
fn typing_keeps_the_cards_whose_title_matches() {
    let here = model(vec![session("a", "web", 3), session("b", "ci", 2)]);
    let machines = [Machine { host: 0, label: "This Mac", model: Some(&here), note: None }];
    let groups = overview::groups(&machines, &Scope::All, false);
    let kept = overview::filter(&groups, "tab b-w1");
    let titles: Vec<_> = kept.iter().flat_map(|g| &g.cards).map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["tab b-w1"]);
    assert_eq!(kept.len(), 1, "a session with no match drops out");
    assert!(overview::filter(&groups, "zzzz").iter().all(|g| g.cards.is_empty()));
    assert_eq!(overview::filter(&groups, "").iter().map(|g| g.cards.len()).sum::<usize>(), 5);
}

#[test]
fn the_grid_fits_the_width_and_never_overlaps() {
    let here = model(vec![session("a", "web", 7), session("b", "ci", 2)]);
    let machines = [Machine { host: 0, label: "This Mac", model: Some(&here), note: None }];
    let groups = overview::groups(&machines, &Scope::All, false);
    for width in [400., 900., 1400., 2600.] {
        let l = overview::layout(&groups, width, 0.6);
        assert_eq!(l.cards.len(), 9);
        assert!(l.cols >= 1);
        for (i, a) in l.cards.iter().enumerate() {
            assert!(a.x >= 0. && a.x + a.w <= width + 0.5, "card {i} inside {width}: {a:?}");
            assert!(a.y + a.h <= l.height + 0.5);
            for b in &l.cards[i + 1..] {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{a:?} overlaps {b:?} at {width}");
            }
        }
        // a session's label sits above its first card
        assert_eq!(l.labels.len(), 2);
        assert!(l.labels[1].0.y + l.labels[1].0.h <= l.cards[7].y);
    }
}

#[test]
fn arrows_move_along_the_order_and_between_rows_by_column() {
    let here = model(vec![session("a", "web", 5), session("b", "ci", 2)]);
    let machines = [Machine { host: 0, label: "This Mac", model: Some(&here), note: None }];
    let groups = overview::groups(&machines, &Scope::All, false);
    // pick a width that gives three columns: rows are [0 1 2] [3 4] | [5 6]
    let l = (300..3000)
        .map(|w| overview::layout(&groups, w as f32, 0.6))
        .find(|l| l.cols == 3)
        .expect("some width gives three columns");
    let s = |from, step| overview::step(&l.cards, from, step);
    assert_eq!(s(0, Step::Left), 0, "clamps at the start");
    assert_eq!(s(0, Step::Right), 1);
    assert_eq!(s(4, Step::Right), 5, "right goes on into the next session");
    assert_eq!(s(6, Step::Right), 6, "clamps at the end");
    assert_eq!(s(1, Step::Down), 4, "same column, next row");
    assert_eq!(s(2, Step::Down), 4, "the nearest column when the row is short");
    assert_eq!(s(4, Step::Down), 6, "across the session's label");
    assert_eq!(s(5, Step::Up), 3);
    assert_eq!(s(6, Step::Down), 6);
    assert_eq!(s(1, Step::Up), 1);
}

#[test]
fn small_sessions_sit_side_by_side_and_a_few_tabs_fill_the_window() {
    let here =
        model(vec![session("a", "api", 1), session("b", "web", 1), session("c", "docs", 1), session("d", "ops", 1)]);
    let machines = [Machine { host: 0, label: "This Mac", model: Some(&here), note: None }];
    let groups = overview::groups(&machines, &Scope::All, false);
    let l = overview::layout(&groups, 1400., 0.6);
    assert!(l.cols >= 4);
    assert!(l.cards.iter().all(|c| c.y == l.cards[0].y), "four one-tab sessions in one row: {:?}", l.cards);
    // the widest cards that fit 1060 px of height: bigger than the 300 px default, never past it
    let f = overview::fit(&groups, 2000., 1060., 0.6);
    assert!(f.height <= 1060., "{}", f.height);
    assert!(f.cards[0].w > 400., "a few tabs get big cards: {:?}", f.cards[0]);
    // many tabs: the smallest cards, and the grid scrolls
    let many = model((0..20).map(|i| session(&format!("s{i}"), "s", 5)).collect());
    let machines = [Machine { host: 0, label: "This Mac", model: Some(&many), note: None }];
    let groups = overview::groups(&machines, &Scope::All, false);
    let f = overview::fit(&groups, 1400., 800., 0.6);
    assert!(f.height > 800. && f.cards[0].w >= 240., "100 tabs scroll at the smallest cards");
}
