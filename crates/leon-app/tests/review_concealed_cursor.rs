//! Review proof: a block cursor never redraws concealed (SGR 8) text in a visible color.

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_app::render::frame;
use leon_app::theme::DARK;

#[test]
fn the_cursor_keeps_concealed_text_hidden() {
    let mut t = Term::new(Config::default(), &TermSize::new(20, 1), VoidListener);
    let mut p: Processor = Processor::new();
    p.advance(&mut t, b"\x1b[8mSECRET\x1b[0m\r");
    let c = frame(&t, &DARK).cursor.expect("cursor at column 0");
    assert_eq!(c.col, 0);
    assert!(c.text.trim().is_empty(), "BUG: concealed text revealed by the cursor: {:?}", c.text);
}
