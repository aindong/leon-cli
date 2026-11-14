//! M19 criterion 9 (.agent/plans/2026-11-15-m19-tmux-migrants.md): what hints match on a screen
//! and the labels they get.

use leon_core::hints::{self, Hint};

fn rows(lines: &[&str]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

fn texts(found: &[Hint]) -> Vec<(usize, usize, &str)> {
    found.iter().map(|h| (h.row, h.col, h.text.as_str())).collect()
}

#[test]
fn the_built_in_rules_find_each_token_once() {
    let screen = rows(&[
        "see https://example.com/a?b=1. and (http://x.org/p)",
        "edit src/main.rs then ~/notes.txt or /usr/local/bin/",
        "commit 3f2a9c1 is 0123456789abcdef0123456789abcdef01234567",
        "id 123e4567-e89b-12d3-a456-426614174000 at 0xdeadBEEF",
        "ping 192.168.1.10:8080 took 12345 ms",
        "hello world 1.2 and 123 ok",
    ]);
    let found = hints::find(&screen, &[]);
    let mut got = texts(&found);
    got.sort();
    assert_eq!(
        got,
        vec![
            (0, 4, "https://example.com/a?b=1"),
            (0, 36, "http://x.org/p"),
            (1, 5, "src/main.rs"),
            (1, 22, "~/notes.txt"),
            (1, 37, "/usr/local/bin/"),
            (2, 7, "3f2a9c1"),
            (2, 18, "0123456789abcdef0123456789abcdef01234567"),
            (3, 3, "123e4567-e89b-12d3-a456-426614174000"),
            (3, 43, "0xdeadBEEF"),
            (4, 5, "192.168.1.10:8080"),
            (4, 28, "12345"),
        ]
    );
    for h in &found {
        assert_eq!(h.cells, h.text.chars().count(), "{h:?} spans its own cells");
    }
}

#[test]
fn columns_count_cells_and_extra_patterns_use_group_one() {
    // a wide character takes two cells: its second cell is \0 in a row of cells
    let screen = rows(&["漢\0 TICKET-42 and id=abc123x"]);
    let extra = [hints::compile(r"TICKET-\d+").unwrap(), hints::compile(r"id=(\w+)").unwrap()];
    let got =
        texts(&hints::find(&screen, &extra)).into_iter().map(|(r, c, t)| (r, c, t.to_string())).collect::<Vec<_>>();
    assert_eq!(got, vec![(0, 3, "TICKET-42".to_string()), (0, 20, "abc123x".to_string())]);
    assert!(hints::compile("(").is_err());
}

#[test]
fn labels_are_unique_prefix_free_and_capped() {
    for n in [1, 2, 25, 26, 27, 100, 600, 676] {
        let l = hints::labels(n);
        assert_eq!(l.len(), n);
        for (i, a) in l.iter().enumerate() {
            assert!(a.chars().all(|c| c.is_ascii_lowercase()), "{a}");
            for b in &l[i + 1..] {
                assert!(!b.starts_with(a.as_str()) && !a.starts_with(b.as_str()), "{a} and {b} for {n}");
            }
        }
        assert!(l.windows(2).all(|w| w[0].len() <= w[1].len()), "shortest first for {n}");
    }
    assert!(hints::labels(26).iter().all(|l| l.len() == 1));
    assert_eq!(hints::labels(1), vec!["a"]);
    assert_eq!(hints::labels(700).len(), 676, "at most 26 × 26");
}

#[test]
fn the_bottom_gets_the_short_labels_and_a_text_keeps_one_label() {
    let mut lines: Vec<String> = (0..30).map(|i| format!("/tmp/file{i}")).collect();
    lines.push("/tmp/file3 again".into());
    let found = hints::find(&lines, &[]);
    assert_eq!(found.len(), 31);
    let label = |row: usize| found.iter().find(|h| h.row == row).unwrap().label.clone();
    assert_eq!(label(30), label(3), "the same text, the same label");
    assert_eq!(label(30).len(), 1, "the bottom row's hint is one letter");
    assert_eq!(label(29).len(), 1);
    assert_eq!(label(0).len(), 2, "the top ones need two");
    let distinct: std::collections::HashSet<_> = found.iter().map(|h| h.label.clone()).collect();
    assert_eq!(distinct.len(), 30, "one label per distinct text");
}
