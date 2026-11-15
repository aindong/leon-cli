//! Audit A2: a span's `cells` comes from the server unchecked (history rows skip `screen::apply`),
//! so the loops over a row's cells are bounded by the row's width, not by `cells`.

use std::sync::mpsc;
use std::time::Duration;

use leon_app::render::{Frame, Span};
use leon_app::{scrollback, stamp};

fn span(col: usize, cells: usize, text: &str) -> Span {
    Span { col, cells, text: text.into(), fg: 0, bold: false, italic: false, underline: false, strikeout: false }
}

/// Run `f` on a thread of its own and fail when it takes over 5 s (a hang otherwise stalls the run).
fn within_5s<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || drop(tx.send(f())));
    rx.recv_timeout(Duration::from_secs(5)).expect("it returned within 5 s")
}

#[test]
fn a_span_claiming_huge_cells_reads_back_within_the_row() {
    let cells = within_5s(|| scrollback::cells(&[span(2, 1_000_000_000_000_000_000, "ab")], 80));
    assert_eq!(cells.len(), 80);
    assert_eq!(cells[2], 'a');
    assert!(cells[3..].iter().all(|c| *c == '\0'), "the rest of the row is the span's: {cells:?}");
    let text = within_5s(|| scrollback::text_of(&[span(0, usize::MAX, "x")], 10, 0, 9));
    assert_eq!(text, "x");
}

#[test]
fn stamping_over_a_span_claiming_huge_cells_returns() {
    let f = within_5s(|| {
        let mut f = Frame { lines: 1, cols: 20, rows: vec![vec![span(3, usize::MAX, "ab")]], ..Frame::default() };
        stamp::stamp(&mut f, 0, 0, "COPY", 1, 2);
        f
    });
    assert_eq!(f.rows[0][0].text, "COPY");
}
