//! Scrollback B2, criterion 1: find reads the store. The same queries over the same output give
//! the same matches as alacritty alone keeping the whole history, across the store's end too.

mod b2_support;

use b2_support::{boundary, plain, split_mid_line};
use leon_core::search::{self, Query};

fn q(query: &str, regex: bool, case_sensitive: bool, limit: usize) -> Query {
    Query { query: query.into(), regex, case_sensitive, limit }
}

#[test]
fn find_matches_what_alacritty_alone_finds() {
    let (t, bytes) = split_mid_line();
    let reference = plain(&bytes);
    let b = boundary(&t);
    let queries = [
        q("needle", false, true, 10_000),
        q("needle", false, false, 10_000),
        q("NEEDLE", false, false, 10_000),
        q("abcd", false, true, 10_000),
        q("efgh", false, true, 10_000),
        q(r"word\d+ other", true, true, 10_000),
        q(r"end$", true, true, 10_000),
        q(r"中文\d", true, true, 10_000),
        q("(in parens", false, true, 10_000),
        q("needle", false, false, 700),
        q("x{70}abcdy", true, true, 10_000),
    ];
    for query in &queries {
        let got = t.find(query).unwrap();
        let want = search::find(&reference, t.scrolled(), query).unwrap();
        assert!(!want.matches.is_empty(), "{:?} finds something", query.query);
        assert_eq!(got.matches, want.matches, "{:?}", query.query);
        assert_eq!(got.truncated, want.truncated, "{:?}", query.query);
    }
    // a match that starts in the store and ends in alacritty's rows is found once
    let across = t.find(&q("abcd", false, true, 10_000)).unwrap();
    let crossing = across.matches.iter().filter(|m| m.start.line < b && m.end.line >= b).count();
    let efgh = t.find(&q("efgh", false, true, 10_000)).unwrap();
    let crossing = crossing + efgh.matches.iter().filter(|m| m.start.line < b && m.end.line >= b).count();
    assert_eq!(crossing, 1, "one match crosses the store's end");
    assert!(across.matches.iter().any(|m| m.end.line < b - 1000), "matches deep in the store");
}
