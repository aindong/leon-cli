//! Scrollback B2, from the review: a click on the screen's part of a line that began in the store,
//! a drag that starts on a bracket at a row's end, and a copy of one stored row that wraps a wide
//! character, each as alacritty alone gives them.

mod b2_support;

use b2_support::{boundary, plain, shown, split_mid_line, ROWS};
use leon_core::search::{self, Kind, Pos};
use leon_core::theme::DARK;

#[test]
fn a_click_just_past_the_store_takes_its_whole_line() {
    let (t, bytes) = split_mid_line();
    let mut reference = plain(&bytes);
    let (b, s) = (boundary(&t), t.scrolled());
    let at = Pos { line: b, col: 2 };
    for kind in [Kind::Line, Kind::Word] {
        let got = t.selection(kind, at, at, true).map(|x| (x.start, x.end, x.text));
        let want = search::select(&mut reference, s, kind, at, at, true).map(|x| (x.start, x.end, x.text));
        assert_eq!(got, want, "{kind:?} click at the store's end");
    }
    assert_eq!(t.link_at(at), search::link_at(&reference, s, at), "a link at the store's end");
}

#[test]
fn a_word_drag_from_a_bracket_widens_as_alacritty_does() {
    let text: String = (0..2000).map(|i| format!("(ab) {i:<66}x (abcde)\r\n")).collect();
    let bytes = text.into_bytes();
    let t = shown(&bytes);
    t.resize(ROWS, 80).unwrap();
    let mut reference = plain(&bytes);
    let s = t.scrolled();
    let l = s - t.lines(0, 0, &DARK).history + 10;
    assert!(l + 3 < boundary(&t), "the lines are in the store");
    let p = |line, col| Pos { line, col };
    for (a, h) in [(p(l, 79), p(l + 2, 6)), (p(l - 2, 10), p(l, 0))] {
        let got = t.selection(Kind::Word, a, h, true).map(|x| (x.start, x.end, x.text));
        let want = search::select(&mut reference, s, Kind::Word, a, h, true).map(|x| (x.start, x.end, x.text));
        assert_eq!(got, want, "bracket end {a:?} to {h:?}");
    }
}

#[test]
fn copying_a_row_that_wraps_a_wide_char_matches_alacritty() {
    let bytes = format!("{}中\r\n", "a".repeat(79)).repeat(2000).into_bytes();
    let t = shown(&bytes);
    t.resize(ROWS, 80).unwrap();
    let mut reference = plain(&bytes);
    let s = t.scrolled();
    let oldest = s - t.lines(0, 0, &DARK).history;
    for r in oldest + 10..oldest + 12 {
        let (a, h) = (Pos { line: r, col: 0 }, Pos { line: r, col: 79 });
        let got = t.selection(Kind::Simple, a, h, true).and_then(|x| x.text);
        let want = search::select(&mut reference, s, Kind::Simple, a, h, true).and_then(|x| x.text);
        assert_eq!(got, want, "row {r}");
    }
}

#[test]
fn an_osc8_link_padded_to_the_edge_spans_as_alacritty_finds_it() {
    // a link filled to the last column, then a line break (not a wrap), then more of the link
    let mut s: String = (0..2000).map(|i| format!("line {i}\r\n")).collect();
    for i in 0..50 {
        s += &format!("\x1b]8;;https://example.com/{i}\x1b\\{}\r\nmore\x1b]8;;\x1b\\ after\r\n", "l".repeat(80));
    }
    let bytes = s.into_bytes();
    let t = shown(&bytes);
    t.resize(ROWS, 80).unwrap();
    let reference = plain(&bytes);
    let (b, sc) = (boundary(&t), t.scrolled());
    for line in b - 60..b - 20 {
        for col in [0, 40, 79] {
            let at = Pos { line, col };
            assert_eq!(t.link_at(at), search::link_at(&reference, sc, at), "line {line} col {col}");
        }
    }
}
