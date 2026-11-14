//! M12 criterion 1: the switcher's rows: most recently used first with no query, the best fuzzy
//! match first with one (recency breaks ties), block rows only when their title matches, and the
//! host on every row, every host's tabs, and each tab's status.

mod m13_support;

use leon_app::status::{self, Indicator};
use leon_app::switcher::{rank, tabs, Row};
use leon_core::program_status::{Kind, State};
use m13_support::*;

fn tab(host: &str, session: &str, tab: &str, window_id: &str) -> Row {
    Row {
        host: 0,
        host_label: host.into(),
        session_id: format!("session:{session}"),
        session: session.into(),
        window_id: window_id.into(),
        tab: tab.into(),
        block_id: None,
        block: None,
        status: None,
    }
}

fn block(of: &Row, block_id: &str, title: &str) -> Row {
    Row { block_id: Some(block_id.into()), block: Some(title.into()), ..of.clone() }
}

fn names(rows: &[Row]) -> Vec<String> {
    rows.iter().map(Row::text).collect()
}

#[test]
fn no_query_lists_every_tab_most_recently_used_first() {
    let tabs = vec![
        tab("This Mac", "web", "vim", "w1"),
        tab("This Mac", "web", "logs", "w2"),
        tab("devbox", "api", "zsh", "w3"),
    ];
    let recent = vec!["w3".to_string(), "w1".to_string()];
    let rows = rank(&tabs, &[], "", &recent);
    assert_eq!(names(&rows), vec!["devbox › api › zsh", "This Mac › web › vim", "This Mac › web › logs"]);
}

#[test]
fn a_query_puts_the_best_match_first_and_recency_breaks_ties() {
    let tabs = vec![
        tab("This Mac", "web", "server log", "w1"),
        tab("This Mac", "docs", "logs", "w2"),
        tab("This Mac", "api", "logs", "w3"),
        tab("This Mac", "misc", "zsh", "w4"),
    ];
    // "logs" scores the same on w2 and w3; w3 was used last
    let rows = rank(&tabs, &[], "logs", &["w3".to_string(), "w2".to_string()]);
    let got = names(&rows);
    assert_eq!(&got[..2], ["This Mac › api › logs", "This Mac › docs › logs"], "{got:?}");
    assert!(!got.iter().any(|n| n.contains("zsh")), "a non-match is left out: {got:?}");
    let rows = rank(&tabs, &[], "api", &[]);
    assert_eq!(names(&rows).first().map(String::as_str), Some("This Mac › api › logs"));
}

#[test]
fn a_block_row_shows_only_when_its_title_matches() {
    let t = tab("This Mac", "web", "zsh", "w1");
    let blocks = vec![block(&t, "b1", "claude: fixing the tests")];
    assert!(
        rank(std::slice::from_ref(&t), &blocks, "", &[]).iter().all(|r| r.block_id.is_none()),
        "no query: tabs only"
    );
    assert!(
        rank(std::slice::from_ref(&t), &blocks, "web", &[]).iter().all(|r| r.block_id.is_none()),
        "web is not its title"
    );
    let rows = rank(std::slice::from_ref(&t), &blocks, "claude", &[]);
    assert_eq!(rows.len(), 1, "{:?}", names(&rows));
    assert_eq!(rows[0].block_id.as_deref(), Some("b1"));
    assert_eq!(rows[0].text(), "This Mac › web › zsh › claude: fixing the tests");
}

#[test]
fn every_row_names_its_host_and_keeps_its_status() {
    let mut t = tab("devbox", "api", "build", "w1");
    t.status = Some(Indicator::Error);
    let rows = rank(&[t], &[], "", &[]);
    assert!(rows[0].text().starts_with("devbox › "));
    assert_eq!(rows[0].status, Some(Indicator::Error));
}

#[test]
fn rows_come_from_every_host_with_each_tabs_status() {
    let here =
        model(vec![("web", vec![Tab(None, "vim", vec![]), Tab(Some("logs"), "tail", vec![record(State::Done)])])]);
    let devbox = model(vec![("api", vec![Tab(None, "claude", vec![blocked(Kind::Question)])])]);
    let (tab_rows, block_rows) = tabs(&[(0, "This Mac", &here), (1, "devbox", &devbox)]);
    let text: Vec<String> = tab_rows.iter().map(Row::text).collect();
    assert_eq!(text, vec!["This Mac › web › vim", "This Mac › web › logs", "devbox › api › claude"]);
    assert_eq!(tab_rows[2].host, 1, "the devbox row is on host 1");
    assert_eq!(tab_rows[2].status, Some(Indicator::Blocked(Some(Kind::Question))));
    assert_eq!(status::glyph(tab_rows[2].status.unwrap(), 0), "?", "the blocked glyph shows its kind");
    assert_eq!(tab_rows[1].status, Some(Indicator::Done));
    assert_eq!(block_rows.len(), 3, "a block row per block, shown only when its title matches");
    let rows = rank(&tab_rows, &block_rows, "tail", &[]);
    assert!(
        rows.iter().any(|r| r.block.as_deref() == Some("tail")),
        "{:?}",
        rows.iter().map(Row::text).collect::<Vec<_>>()
    );
}
