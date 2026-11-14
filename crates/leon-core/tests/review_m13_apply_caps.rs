//! Review of M13: a row a server sends brings at most a line's worth of spans and text, so a
//! frame stays the size of a screen whatever arrives.

use leon_core::screen::{apply, Frame, Row, Span, Update};

fn span(col: usize, text: String) -> Span {
    Span { col, cells: 1, text, fg: 0, bold: false, italic: false, underline: false, strikeout: false }
}

fn update(line: usize, spans: Vec<Span>) -> Update {
    Update {
        full: false,
        lines: 2,
        cols: 10,
        rows: vec![Row { line, spans, backgrounds: Vec::new() }],
        cursor: None,
        app_cursor: false,
        bracketed_paste: false,
        view: Default::default(),
    }
}

#[test]
fn a_row_brings_at_most_a_lines_worth_of_spans_and_text() {
    let mut f = Frame { lines: 2, cols: 10, rows: vec![Vec::new(); 2], ..Frame::default() };
    apply(&mut f, update(1, (0..100_000).map(|i| span(i, "x".into())).collect()));
    assert!(f.rows[1].len() <= 10, "{} spans kept for a 10-column line", f.rows[1].len());
    apply(&mut f, update(0, vec![span(0, "x".repeat(1 << 20))]));
    let chars: usize = f.rows[0].iter().map(|s| s.text.chars().count()).sum();
    assert!(chars <= 10 * 17, "{chars} chars kept for a 10-column line");
    apply(&mut f, update(0, vec![span(2, "é".into())]));
    assert_eq!(f.rows[0][0].text, "é", "ordinary text is untouched");
}
