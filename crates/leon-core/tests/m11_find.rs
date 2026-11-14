//! M11 criterion 4: find across scrollback and screen (`Terminal::find`).

use leon_core::search::{Pos, Query};
use leon_core::terminal::{Spec, Terminal};

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

fn q(query: &str, regex: bool, case_sensitive: bool) -> Query {
    Query { query: query.into(), regex, case_sensitive, limit: 1000 }
}

fn at(line: usize, col: usize) -> Pos {
    Pos { line, col }
}

/// 60 lines: "needle" on lines 5 and 40, "Needle" on 30, the rest filler; 60 > 24 rows, so 5 and
/// 30 are in the scrollback.
fn haystack() -> String {
    (0..60)
        .map(|i| match i {
            5 | 40 => format!("a needle {i}\r\n"),
            30 => "Needle here\r\n".into(),
            _ => format!("hay {i}\r\n"),
        })
        .collect()
}

#[test]
fn literal_matches_across_scrollback_and_screen() {
    let t = shown(haystack().as_bytes());
    let found = t.find(&q("needle", false, true)).unwrap();
    let starts: Vec<Pos> = found.matches.iter().map(|m| m.start).collect();
    assert_eq!(starts, vec![at(5, 2), at(40, 2)], "case-sensitive: not Needle");
    assert_eq!(found.matches[0].end, at(5, 7));
    assert!(!found.truncated);
    let any_case = t.find(&q("needle", false, false)).unwrap();
    assert_eq!(any_case.matches.len(), 3, "case-insensitive finds Needle too");
    let upper = t.find(&q("Needle", false, false)).unwrap();
    assert_eq!(upper.matches.len(), 3, "an upper-case query still ignores case when told to");
}

#[test]
fn literal_text_is_not_a_pattern() {
    let t = shown(b"abc a.c\r\n");
    assert_eq!(t.find(&q("a.c", false, true)).unwrap().matches.len(), 1);
    assert_eq!(t.find(&q("a.c", true, true)).unwrap().matches.len(), 2);
}

#[test]
fn regex_queries_and_bad_ones() {
    let t = shown(haystack().as_bytes());
    let found = t.find(&q(r"hay 1\d$", true, true)).unwrap();
    assert_eq!(found.matches.len(), 10, "hay 10 … hay 19");
    assert!(t.find(&q("(", true, true)).is_err(), "a bad regex is an error");
}

#[test]
fn a_match_across_a_wrap_is_one_match() {
    let text = format!("{}needle\r\n", "x".repeat(76));
    let t = shown(text.as_bytes());
    let found = t.find(&q("needle", false, true)).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!((found.matches[0].start, found.matches[0].end), (at(0, 76), at(1, 1)));
}

#[test]
fn the_limit_holds_and_empty_matches_end() {
    let t = shown(haystack().as_bytes());
    let found = t.find(&Query { limit: 2, ..q("hay", false, true) }).unwrap();
    assert_eq!(found.matches.len(), 2);
    assert!(found.truncated);
    let empty = t.find(&q("x*", true, true)).unwrap();
    assert!(empty.matches.len() <= 1000, "a pattern that matches nothing at every cell still returns");
}

#[test]
fn an_end_anchor_matches_before_the_padding_and_not_in_a_class() {
    let t = shown(b"ends here\r\na$b\r\n");
    let found = t.find(&q("here$", true, true)).unwrap();
    assert_eq!(found.matches.len(), 1);
    assert_eq!(found.matches[0].end, at(0, 8), "the padding isn't part of the match");
    assert_eq!(t.find(&q(r"a[$]b", true, true)).unwrap().matches.len(), 1, "a $ in a class is a dollar");
    assert_eq!(t.find(&q(r"a\$b", true, true)).unwrap().matches.len(), 1, "an escaped $ too");
}
