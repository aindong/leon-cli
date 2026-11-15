//! Daily-driver A1 (docs/research/daily-driver-0.1.0.md): among equal ⌘K matches, what needs the
//! user comes first (blocked, error, done), then recency; a better match still wins, and with no
//! query the order is the recency order it always was.

use leon_app::status::Indicator;
use leon_app::switcher::{rank, Row};
use leon_core::program_status::Kind;

fn tab(session: &str, tab: &str, window_id: &str, status: Option<Indicator>) -> Row {
    Row {
        host: 0,
        host_label: "This Mac".into(),
        session_id: format!("session:{session}"),
        session: session.into(),
        window_id: window_id.into(),
        tab: tab.into(),
        block_id: None,
        block: None,
        status,
    }
}

fn names(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| r.tab.clone()).collect()
}

#[test]
fn the_failed_agent_ranks_above_the_idle_shell_it_ties_with() {
    let tabs = vec![tab("web", "zsh", "w1", None), tab("web", "fake-agent", "w2", Some(Indicator::Error))];
    // the shell was used last; "web" matches both through the session name alike
    let rows = rank(&tabs, &[], "web", &["w1".into(), "w2".into()]);
    assert_eq!(names(&rows), ["fake-agent", "zsh"]);
}

#[test]
fn blocked_then_error_then_done_then_the_rest_by_recency() {
    let tabs = vec![
        tab("api", "a", "w1", None),
        tab("api", "b", "w2", Some(Indicator::Done)),
        tab("api", "c", "w3", Some(Indicator::Working { progress: None })),
        tab("api", "d", "w4", Some(Indicator::Error)),
        tab("api", "e", "w5", Some(Indicator::Blocked(Some(Kind::Question)))),
        tab("api", "f", "w6", None),
    ];
    let recent: Vec<String> = ["w6", "w1", "w3"].map(String::from).to_vec();
    let rows = rank(&tabs, &[], "api", &recent);
    assert_eq!(names(&rows), ["e", "d", "b", "f", "a", "c"]);
}

#[test]
fn a_better_match_beats_a_worse_one_that_needs_you() {
    let tabs = vec![tab("misc", "deploy", "w1", None), tab("misc", "deep sea ploy", "w2", Some(Indicator::Error))];
    let rows = rank(&tabs, &[], "deploy", &[]);
    assert_eq!(names(&rows).first().map(String::as_str), Some("deploy"));
}

#[test]
fn no_query_keeps_the_most_recently_used_order() {
    let tabs = vec![tab("web", "zsh", "w1", None), tab("web", "agent", "w2", Some(Indicator::Error))];
    let rows = rank(&tabs, &[], "", &["w1".into()]);
    assert_eq!(names(&rows), ["zsh", "agent"]);
}

#[test]
fn a_block_that_needs_you_beats_an_idle_tab_it_ties_with() {
    let shell = tab("web", "zsh", "w1", None);
    let agent = Row {
        block_id: Some("b2".into()),
        block: Some("web tests".into()),
        status: Some(Indicator::Blocked(None)),
        ..tab("web", "zsh", "w1", None)
    };
    // "web" scores the same on the tab's session and the block's title
    let rows = rank(&[shell], &[agent], "web", &[]);
    assert_eq!(rows.iter().map(|r| r.block_id.is_some()).collect::<Vec<_>>(), [true, false]);
}
