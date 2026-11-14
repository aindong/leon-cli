//! Review proof: underlined trailing spaces keep their underline; default blanks after stay plain.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_app::render::frame;
use leon_app::theme::DARK;

#[test]
fn underlined_trailing_spaces_keep_their_underline() {
    let mut t = Term::new(Config::default(), &TermSize::new(20, 1), VoidListener);
    let mut p: Processor = Processor::new();
    p.advance(&mut t, b"\x1b[4mab   \x1b[0m");
    let f = frame(&t, &DARK);
    let under = |col: usize| f.rows[0].iter().any(|s| s.underline && (s.col..s.col + s.cells).contains(&col));
    assert!(under(4), "BUG: underlined trailing spaces lost: {:?}", f.rows[0]);
    assert!(!under(5), "default blanks after it stay plain: {:?}", f.rows[0]);
}
