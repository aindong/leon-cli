//! Scrollback B2, criterion 3: a link in a line that moved into the store opens as it did: an
//! OSC 8 link from the cell, a URL from the text.

mod b2_support;

use b2_support::{boundary, plain, split_mid_line};
use leon_core::screen::row_text;
use leon_core::search::{self, Pos};
use leon_core::theme::DARK;

#[test]
fn links_in_the_store_match_what_alacritty_alone_finds() {
    let (t, bytes) = split_mid_line();
    let reference = plain(&bytes);
    let (b, s) = (boundary(&t), t.scrolled());
    let oldest = s - t.lines(0, 0, &DARK).history;
    let (mut osc8, mut url) = (0, 0);
    for line in oldest..oldest + 60 {
        let text = t.lines(line, 1, &DARK).rows.first().map(|r| row_text(&r.spans)).unwrap_or_default();
        for col in [0, 3, 9, 15, 25, 40] {
            let at = Pos { line, col };
            let (got, want) = (t.link_at(at), search::link_at(&reference, s, at));
            assert_eq!(got, want, "line {line} ({text:?}) col {col}");
            match want {
                Some(l) if l.uri.starts_with("file:") => osc8 += 1,
                Some(_) => url += 1,
                None => {}
            }
        }
    }
    assert!(oldest + 60 < b, "the lines read are in the store");
    assert!(osc8 > 0 && url > 0, "both kinds were found: {osc8} OSC 8, {url} URLs");
}
