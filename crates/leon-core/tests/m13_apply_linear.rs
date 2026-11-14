//! M13 criterion 10: applying a screen update costs time in proportion to its size and the
//! frame's, not their product, whatever a (remote, maybe hostile) server puts in it.

use std::sync::mpsc;
use std::time::Duration;

use leon_core::screen::{apply, Bg, BgRect, Frame, Row, Update};

fn update(rows: Vec<Row>) -> Update {
    Update {
        full: false,
        lines: 500,
        cols: 1000,
        rows,
        cursor: None,
        app_cursor: false,
        bracketed_paste: false,
        view: Default::default(),
    }
}

#[test]
fn many_rows_over_many_backgrounds_apply_in_linear_time() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut f = Frame { lines: 500, cols: 1000, rows: vec![Vec::new(); 500], ..Frame::default() };
        f.backgrounds = (0..100_000).map(|i| BgRect { line: i % 500, col: i % 1000, len: 1, color: 1 }).collect();
        // the same line again and again: a quadratic apply does 10^10 comparisons here
        let rows = (0..100_000)
            .map(|i| Row { line: 0, spans: Vec::new(), backgrounds: vec![Bg { col: i % 1000, len: 1, color: 2 }] })
            .collect();
        apply(&mut f, update(rows));
        let _ = tx.send(f);
    });
    let f = rx.recv_timeout(Duration::from_secs(2)).expect("apply finished within the deadline");
    let line0: Vec<&BgRect> = f.backgrounds.iter().filter(|b| b.line == 0).collect();
    assert_eq!(line0.len(), 1, "line 0 keeps only its last row's backgrounds");
    assert_eq!((line0[0].col, line0[0].color), (99_999 % 1000, 2));
    assert_eq!(f.backgrounds.len(), 1 + 100_000 - 200, "other lines keep theirs");
}

#[test]
fn a_row_brings_at_most_a_lines_worth_of_backgrounds() {
    let mut f = Frame { lines: 2, cols: 10, rows: vec![Vec::new(); 2], ..Frame::default() };
    let flood = (0..100_000).map(|i| Bg { col: i, len: 1, color: 3 }).collect();
    apply(&mut f, update(vec![Row { line: 1, spans: Vec::new(), backgrounds: flood }]));
    assert!(f.backgrounds.len() <= 10, "{} backgrounds for a 10-column line", f.backgrounds.len());
}
