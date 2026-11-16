//! A frame's copy of the screen shares no cell's mark storage with the emulator: if it did, the
//! emulator's next mark on that cell would copy all of them (`Arc::make_mut`) on the parse thread,
//! and a program can stack any number of marks on one cell.

use std::sync::Arc;

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::index::{Column, Line};
use alacritty_terminal::term::{test::TermSize, Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_core::screen::{build, Shot};
use leon_core::theme::DARK;

#[test]
fn a_shot_does_not_share_a_cells_marks_with_the_emulator() {
    let mut term = Term::new(Config::default(), &TermSize::new(80, 24), VoidListener);
    let mut parser: Processor = Processor::new();
    let mut input = String::from("a");
    input.extend(std::iter::repeat_n('\u{301}', 1000));
    parser.advance(&mut term, input.as_bytes());
    let shot = Shot::of(&term);
    let extra = term.grid()[Line(0)][Column(0)].extra.as_ref().expect("the marks are stored on the cell");
    assert_eq!(Arc::strong_count(extra), 1, "Shot holds the cell's mark storage");
    // the frame still draws the first marks, as many as it ever did
    let f = build(&shot, &DARK);
    assert_eq!(f.rows[0][0].text, format!("a{}", "\u{301}".repeat(16)));
}
