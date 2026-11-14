//! Criterion 1: an alacritty grid turns into the render model the view paints.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_app::render::{frame, Frame, Span};
use leon_app::theme::{indexed, DARK};

fn term(cols: usize, lines: usize, bytes: &[u8]) -> Term<VoidListener> {
    let mut t = Term::new(Config::default(), &TermSize::new(cols, lines), VoidListener);
    let mut p: Processor = Processor::new();
    p.advance(&mut t, bytes);
    t
}

fn span<'a>(f: &'a Frame, line: usize, text: &str) -> &'a Span {
    f.rows[line].iter().find(|s| s.text == text).unwrap_or_else(|| panic!("no span {text:?} in {:?}", f.rows[line]))
}

fn bg_at(f: &Frame, line: usize, col: usize) -> Option<u32> {
    f.backgrounds.iter().find(|r| r.line == line && (r.col..r.col + r.len).contains(&col)).map(|r| r.color)
}

#[test]
fn styles_and_colors() {
    let t = term(40, 3, b"\x1b[1;31mR\x1b[0m \x1b[3;4mI\x1b[0m \x1b[7mV\x1b[0m \x1b[38;2;1;2;3mT\x1b[48;5;196mB\x1b[0m");
    let f = frame(&t, &DARK);
    let r = span(&f, 0, "R");
    assert!(r.bold && !r.italic && !r.underline);
    assert_eq!((r.col, r.fg), (0, DARK.ansi[1]));
    let i = span(&f, 0, "I");
    assert!(i.italic && i.underline && !i.bold);
    assert_eq!(i.col, 2);
    let v = span(&f, 0, "V");
    assert_eq!(v.fg, DARK.bg, "inverse draws the text in the background color");
    assert_eq!(bg_at(&f, 0, 4), Some(DARK.fg), "on a foreground-colored cell");
    assert_eq!(span(&f, 0, "T").fg, 0x010203);
    assert_eq!(bg_at(&f, 0, 7), Some(0xff0000), "palette 196");
    assert_eq!(bg_at(&f, 0, 6), None, "default background paints nothing");
    assert!(f.rows[1].is_empty() && f.rows[2].is_empty(), "blank lines make no spans");
}

#[test]
fn palette_256() {
    assert_eq!(indexed(21, &DARK), 0x0000ff);
    assert_eq!(indexed(244, &DARK), 0x808080);
    assert_eq!(indexed(196, &DARK), 0xff0000);
    assert_eq!(indexed(3, &DARK), DARK.ansi[3]);
}

#[test]
fn wide_chars_take_two_columns() {
    let t = term(20, 2, "中x".as_bytes());
    let f = frame(&t, &DARK);
    let wide = f.rows[0].iter().find(|s| s.text.starts_with('中')).expect("wide span");
    assert_eq!(wide.col, 0);
    let x = f.rows[0].iter().find(|s| s.text.contains('x')).expect("x span");
    assert_eq!(x.col + x.text.find('x').unwrap(), 2, "x is drawn at column 2");
}

#[test]
fn cursor_follows_the_terminal_and_hides() {
    let f = frame(&term(20, 4, b"ab\r\ncd"), &DARK);
    let c = f.cursor.expect("cursor shown");
    assert_eq!((c.line, c.col), (1, 2));
    assert!(frame(&term(20, 4, b"ab\x1b[?25l"), &DARK).cursor.is_none());
}

#[test]
fn trailing_blanks_make_no_spans() {
    let f = frame(&term(20, 2, b"hi"), &DARK);
    assert_eq!(f.rows[0].len(), 1);
    assert_eq!(f.rows[0][0].text, "hi");
    assert_eq!((f.lines, f.cols), (2, 20));
}
