//! Scrollback B2's reference: the same output fed to a terminal (most of its history in leon's
//! store) and to a plain alacritty `Term` that keeps all of it, numbered alike.

#![allow(dead_code)] // each test file uses part of it

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

pub struct Size(pub usize);

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.0
    }
    fn screen_lines(&self) -> usize {
        self.0
    }
    fn columns(&self) -> usize {
        80
    }
}

/// The screen's rows after `split_mid_line`'s resize, which pushes 14 of the 24 into alacritty's
/// history: lines in the store, in alacritty's history and on the screen.
pub const ROWS: u16 = 10;

/// alacritty alone, keeping 10,000 lines, resized as `split_mid_line` resizes.
pub fn plain(bytes: &[u8]) -> Term<VoidListener> {
    let mut term = Term::new(Config { scrolling_history: 10_000, ..Config::default() }, &Size(24), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, bytes);
    term.resize(Size(ROWS.into()));
    term
}

pub fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

/// The first line alacritty itself holds; older ones are in the store.
pub fn boundary(t: &Terminal) -> usize {
    t.scrolled() - t.with_term(|t| t.grid().history_size())
}

/// 2,000 short lines of words, brackets, wide characters, tabs and links, then 1,500 lines of
/// 230 characters that wrap twice with `abcd` and `efgh` across the wraps, then `pad` short
/// lines (to move where the store ends).
pub fn output(pad: usize) -> Vec<u8> {
    let mut s = String::new();
    for i in 0..2000 {
        s += &match i % 6 {
            0 => format!("word{i} other (in parens {i}) [square] end"),
            1 => format!("wide 中文{i} text\tafter tab"),
            2 => format!("see https://example.com/path/{i}). and more"),
            3 => format!("\x1b]8;;file:///tmp/f{i}\x1b\\linked {i}\x1b]8;;\x1b\\ plain"),
            4 => format!("needle {i} NEEDLE needle"),
            _ => "w".repeat(i % 90),
        };
        s += "\r\n";
    }
    for i in 0..1500 {
        s += &format!("{i:05}{}abcd{}efgh{}\r\n", "x".repeat(73), "y".repeat(76), "z".repeat(68));
    }
    for i in 0..pad {
        s += &format!("pad {i}\r\n");
    }
    s.into_bytes()
}

/// A terminal shown `output(pad)` whose store ends inside a wrapped line (the row before the
/// boundary is a full one), and the pad that gave it.
pub fn split_mid_line() -> (Terminal, Vec<u8>) {
    for pad in 0..3 {
        let bytes = output(pad);
        let t = shown(&bytes);
        t.resize(ROWS, 80).unwrap();
        let b = boundary(&t);
        assert_eq!(t.scrolled() - b, 14, "alacritty holds the screen's 14 rows pushed by the resize");
        let before = t.lines(b - 1, 1, &DARK).rows.first().map(|r| row_text(&r.spans).chars().count());
        let l = t.lines(0, 0, &DARK);
        assert!(b > l.scrolled - l.history + 1000, "the store holds lines");
        if before == Some(80) {
            return (t, bytes);
        }
    }
    panic!("no pad put the store's end inside a wrapped line");
}
