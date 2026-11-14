//! Review proof: a plain space between two underlined (or struck-out) words is not drawn underlined.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_app::render::frame;
use leon_app::theme::DARK;

fn term(cols: usize, lines: usize, bytes: &[u8]) -> Term<VoidListener> {
    let mut t = Term::new(Config::default(), &TermSize::new(cols, lines), VoidListener);
    let mut p: Processor = Processor::new();
    p.advance(&mut t, bytes);
    t
}

fn covers(col: usize, s: &leon_app::render::Span) -> bool {
    (s.col..s.col + s.cells).contains(&col)
}

#[test]
fn a_plain_gap_between_underlined_words_is_not_underlined() {
    let f = frame(&term(20, 1, b"\x1b[4mA\x1b[0m \x1b[4mB\x1b[0m"), &DARK);
    let bad: Vec<_> = f.rows[0].iter().filter(|s| s.underline && covers(1, s)).collect();
    assert!(bad.is_empty(), "underlined span covers the plain space at col 1: {bad:?}");
}

#[test]
fn a_plain_gap_between_struck_words_is_not_struck() {
    let f = frame(&term(20, 1, b"\x1b[9mA\x1b[0m \x1b[9mB\x1b[0m"), &DARK);
    let bad: Vec<_> = f.rows[0].iter().filter(|s| s.strikeout && covers(1, s)).collect();
    assert!(bad.is_empty(), "struck-out span covers the plain space at col 1: {bad:?}");
}

/// The painter pins glyph i of a span to column i, which is only right when each char is a cell.
#[test]
fn spans_hold_one_char_per_cell_or_stand_alone() {
    let f = frame(&term(20, 1, "ab e\u{301}cd 中ef".as_bytes()), &DARK);
    for s in &f.rows[0] {
        let n = s.text.chars().count();
        let lone = s.cells == 1 || (s.cells == 2 && n == 1); // a marked cell, or one wide char
        assert!(n == s.cells || lone, "a cell with marks or a wide char stands alone: {s:?}");
    }
    let cd = f.rows[0].iter().find(|s| s.text.starts_with("cd")).expect("cd after the marked cell");
    assert_eq!(cd.col, 4);
}
