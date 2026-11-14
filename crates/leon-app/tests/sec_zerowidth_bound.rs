//! Review proof: combining marks on one cell can't make a frame's work unbounded.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_app::render::frame;
use leon_app::theme::DARK;

#[test]
fn combining_marks_per_cell_are_bounded() {
    let mut t = Term::new(Config::default(), &TermSize::new(80, 24), VoidListener);
    let mut p: Processor = Processor::new();
    let mut out = String::from("a");
    out.extend(std::iter::repeat_n('\u{301}', 200_000));
    p.advance(&mut t, out.as_bytes());
    let f = frame(&t, &DARK);
    let longest = f.rows.iter().flatten().map(|s| s.text.chars().count()).max().unwrap_or(0);
    assert!(longest <= 1_000, "unbounded zerowidth: one cell rendered as {longest} chars");
}
