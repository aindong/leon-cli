//! Review proof: the cursor stays visible on a leading wide-char spacer.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_app::render::frame;
use leon_app::theme::DARK;

#[test]
fn cursor_on_a_leading_wide_char_spacer_is_drawn() {
    let mut t = Term::new(Config::default(), &TermSize::new(10, 3), VoidListener);
    let mut p: Processor = Processor::new();
    // `中` doesn't fit in column 9, so col 9 becomes a leading spacer; then CUP back onto it.
    p.advance(&mut t, "abcdefghi中\x1b[1;10H".as_bytes());
    let c = frame(&t, &DARK).cursor.expect("BUG: cursor vanished on a leading wide-char spacer");
    assert_eq!((c.line, c.col), (0, 9));
}
