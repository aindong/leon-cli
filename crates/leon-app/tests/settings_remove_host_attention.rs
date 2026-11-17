//! Removing a host in Settings moves the hosts after it up one: what the attention tracker knows
//! about them moves with them, so their blocks aren't news again (found in review).
mod m13_support;

use std::time::Instant;

use leon_app::attention::Tracker;
use leon_core::program_status::Kind;
use m13_support::*;

#[test]
fn removing_a_host_does_not_make_the_next_hosts_blocks_news() {
    let here = model(vec![]);
    let a = model(vec![("a", vec![Tab(None, "claude", vec![blocked(Kind::Question)])])]);
    let b = model(vec![("b", vec![Tab(None, "claude", vec![blocked(Kind::Question)])])]);
    let mut t = Tracker::default();
    let now = Instant::now();
    t.update(&[(0, "This Mac", &here), (1, "alpha", &a), (2, "beta", &b)], now);
    // Settings › Remote Hosts removes alpha (Workspace::remove_host): beta moves up to index 1
    t.forget_host(1);
    let rows: Vec<(usize, String)> = t.rows().iter().map(|r| (r.host, r.session_id.clone())).collect();
    assert!(rows.iter().all(|(h, _)| *h == 1), "alpha's row is gone and beta's moved up: {rows:?}");
    assert_eq!(rows.len(), 1, "{rows:?}");
    let changes = t.update(&[(0, "This Mac", &here), (1, "beta", &b)], now);
    assert!(changes.is_empty(), "beta's block was already blocked: {changes:?}");
}
