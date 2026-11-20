//! Scrollback B2, from the review: a word or line selection whose end lands on column 0 of a
//! stored line that starts with a bracket, or whose start is a bracket at a row's end, widens as
//! alacritty alone widens it (to the word), not to the matching bracket as a double-click does.

mod b2_support;

use b2_support::{boundary, plain, shown, ROWS};
use leon_core::search::{self, Kind, Pos};
use leon_core::theme::DARK;

#[test]
fn a_drag_onto_a_bracket_widens_as_alacritty_does() {
    let mut s: String = (0..3000).map(|i| format!("(a b) tail {i}\r\n")).collect();
    s += &(0..50).map(|i| format!("{}(x) {i:03} [y]\r\n", "z".repeat(70))).collect::<String>();
    let bytes = s.into_bytes();
    let t = shown(&bytes);
    t.resize(ROWS, 80).unwrap();
    let mut reference = plain(&bytes);
    let (b, sc) = (boundary(&t), t.scrolled());
    let oldest = sc - t.lines(0, 0, &DARK).history;
    let l = oldest + 10;
    assert!(l + 60 < b, "the lines are in the store (boundary {b})");
    let at = |line, col| Pos { line, col };
    for kind in [Kind::Word, Kind::Line, Kind::Simple] {
        let last_row = b - 20; // a `…(x) 0NN [y]` row whose last column is `]`
        for (a, h) in [(at(l - 3, 2), at(l, 0)), (at(last_row, 79), at(last_row + 2, 5)), (at(l, 0), at(l + 4, 1))] {
            let got = t.selection(kind, a, h, true).expect("a selection");
            let want = search::select(&mut reference, sc, kind, a, h, true).unwrap();
            assert_eq!((got.start, got.end), (want.start, want.end), "{kind:?} {a:?} {h:?}: range");
            assert_eq!(got.text, want.text, "{kind:?} {a:?} {h:?}: text");
        }
    }
}
