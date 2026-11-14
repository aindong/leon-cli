//! M11 criterion 2: selection geometry and the text it copies (`Terminal::selection`), in
//! absolute line numbers: wrapped lines, rectangles, wide characters, words and lines.

use leon_core::search::{Kind, Pos};
use leon_core::terminal::{Spec, Terminal};

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

fn at(line: usize, col: usize) -> Pos {
    Pos { line, col }
}

fn copy(t: &Terminal, kind: Kind, a: Pos, b: Pos) -> String {
    t.selection(kind, a, b, true).expect("a selection").text.expect("its text")
}

/// 100 characters: wider than the 80-column screen, so it wraps once.
fn long() -> String {
    (0..100).map(|i| char::from(b'a' + (i % 26) as u8)).collect()
}

#[test]
fn a_wrapped_line_copies_as_one_line() {
    let t = shown(format!("{}\r\nnext", long()).as_bytes());
    assert_eq!(copy(&t, Kind::Simple, at(0, 0), at(1, 19)), long(), "no newline where it wrapped");
    assert_eq!(copy(&t, Kind::Simple, at(0, 78), at(2, 3)), format!("{}\nnext", &long()[78..]));
    // dragged backwards: the same text
    assert_eq!(copy(&t, Kind::Simple, at(1, 19), at(0, 0)), long());
}

#[test]
fn a_rectangle_copies_its_columns() {
    let t = shown(b"abcdef\r\nghijkl\r\nmnopqr");
    assert_eq!(copy(&t, Kind::Block, at(0, 1), at(2, 3)), "bcd\nhij\nnop");
    assert_eq!(copy(&t, Kind::Block, at(2, 3), at(0, 1)), "bcd\nhij\nnop", "from any corner");
}

#[test]
fn half_a_wide_character_takes_the_whole_one() {
    // a, then 中 in columns 1–2, then b in column 3
    let t = shown("a中b".as_bytes());
    assert_eq!(copy(&t, Kind::Simple, at(0, 2), at(0, 3)), "中b", "starting on its right half");
    assert_eq!(copy(&t, Kind::Simple, at(0, 0), at(0, 1)), "a中", "ending on its left half");
    let s = t.selection(Kind::Simple, at(0, 2), at(0, 3), false).unwrap();
    assert_eq!((s.start, s.end, s.text), (at(0, 1), at(0, 3), None), "the range covers both cells");
}

#[test]
fn a_word_and_a_logical_line() {
    let t = shown(format!("hello world foo\r\n{}\r\nlast", long()).as_bytes());
    let w = t.selection(Kind::Word, at(0, 7), at(0, 7), true).unwrap();
    assert_eq!((w.start, w.end, w.text.as_deref()), (at(0, 6), at(0, 10), Some("world")));
    let l = t.selection(Kind::Line, at(2, 5), at(2, 5), true).unwrap();
    assert_eq!(l.start, at(1, 0), "a line selection reaches back over the wrap");
    assert_eq!(l.end.line, 2);
    assert_eq!(l.text.unwrap().trim_end(), long());
}

#[test]
fn selections_reach_into_the_scrollback() {
    let lines: String = (0..100).map(|i| format!("line{i}\r\n")).collect();
    let t = shown(lines.as_bytes());
    // 100 lines on a 24-row screen: line 10 is deep in the scrollback
    assert_eq!(copy(&t, Kind::Simple, at(10, 0), at(11, 5)), "line10\nline11");
    assert!(t.selection(Kind::Simple, at(1_000_000, 0), at(1_000_001, 0), true).is_none(), "beyond the screen");
}
