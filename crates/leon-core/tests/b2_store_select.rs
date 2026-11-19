//! Scrollback B2, criterion 2: selections with an end in the store give the same range and text
//! as alacritty alone keeping the whole history, for every kind, across the store's end too.

mod b2_support;

use b2_support::{boundary, plain, split_mid_line};
use leon_core::search::{self, Kind, Pos};
use leon_core::theme::DARK;

fn at(line: usize, col: usize) -> Pos {
    Pos { line, col }
}

#[test]
fn selections_match_what_alacritty_alone_selects() {
    let (t, bytes) = split_mid_line();
    let mut reference = plain(&bytes);
    let (b, s) = (boundary(&t), t.scrolled());
    let oldest = s - t.lines(0, 0, &DARK).history;
    let short = oldest + 6; // a `word… other (in parens …) [square] end` line is within 6 of any start
    let mut cases = vec![];
    for kind in [Kind::Simple, Kind::Word, Kind::Line, Kind::Block] {
        cases.push((kind, at(short, 3), at(short + 40, 10))); // within the store
        cases.push((kind, at(b - 30, 70), at(b + 5, 4))); // across the store's end
        cases.push((kind, at(b - 1, 79), at(b, 2))); // the boundary's two rows
        cases.push((kind, at(oldest + 100, 0), at(s + 3, 20))); // into the screen
        cases.push((kind, at(s + 5, 5), at(short + 2, 1))); // dragged backwards
    }
    for line in short..short + 6 {
        for col in [0, 3, 12, 17, 26, 27, 33, 38] {
            cases.push((Kind::Word, at(line, col), at(line, col))); // a double-click: words, brackets
            cases.push((Kind::Line, at(line, col), at(line, col)));
        }
    }
    cases.push((Kind::Word, at(b - 1, 79), at(b - 1, 79))); // a word across the store's end
    cases.push((Kind::Simple, at(oldest, 0), at(s + 9, 79))); // select all
    for (kind, a, h) in cases {
        let got = t.selection(kind, a, h, true).unwrap_or_else(|| panic!("{kind:?} {a:?} {h:?}: a selection"));
        let want = search::select(&mut reference, s, kind, a, h, true).unwrap();
        let what = format!("{kind:?} from {a:?} to {h:?}");
        assert_eq!((got.start, got.end), (want.start, want.end), "{what}: range");
        assert_eq!(got.text, want.text, "{what}: text");
    }
    let all = t.selection(Kind::Simple, at(oldest, 0), at(s + 9, 79), true).unwrap().text.unwrap();
    assert!(all.lines().count() >= 3000 && all.contains("word0 other") == (oldest == 0), "select-all copies it all");
}
