//! Scrollback B1, criterion 4: a resize leaves stored lines as they were written. alacritty
//! re-wraps its own rows; the store's keep their text and order, are cut at the pane's width in
//! `lines` and are whole in `text`.

use alacritty_terminal::grid::Dimensions;
use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

/// 5,000 lines of 70 characters, each starting `line-NNNN-`.
fn long(n: usize) -> String {
    format!("line-{n:04}-{}", "x".repeat(60))
}

#[test]
fn stored_lines_keep_their_text_across_resizes() {
    let bytes: String = (0..5000).map(|n| long(n) + "\r\n").collect();
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_showing(spec, bytes.as_bytes(), |_| {}).unwrap();
    let all: Vec<String> = (0..5000).map(long).collect();
    let text_lines = |t: &Terminal| t.text(true).lines().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(text_lines(&t), all, "before");

    for (rows, cols) in [(24, 40), (10, 120), (30, 80)] {
        t.resize(rows, cols).unwrap();
        let what = format!("at {rows}x{cols}");
        assert_eq!(text_lines(&t), all, "{what}: text keeps every line whole and in order");
        let l = t.lines(0, 0, &DARK);
        assert!(l.scrolled >= l.history, "{what}: scrolled {} under history {}", l.scrolled, l.history);
        let oldest = l.scrolled - l.history;
        let rows = t.lines(oldest, 200, &DARK).rows;
        assert_eq!(rows.len(), 200, "{what}: the stored rows are kept");
        for (i, r) in rows.iter().enumerate() {
            // stored rows aren't re-wrapped: row i is line i, cut at the pane's width
            let want: String = long(i).chars().take(cols as usize).collect();
            assert_eq!(row_text(&r.spans), want, "{what}: stored row {i}");
            assert!(r.spans.iter().all(|s| s.col + s.cells <= cols as usize), "{what}: row {i} fits the pane");
        }
    }
}

#[test]
fn narrowing_a_wide_window_loses_no_line() {
    // 3,000 lines of 200 columns, most of them in alacritty's window: at 30 columns its 1,000
    // rows re-wrap to about 7,000, more than alacritty keeps
    let wide = |n: usize| format!("wide-{n:04}-{}", "y".repeat(189));
    let bytes: String = (0..3000).map(|n| wide(n) + "\r\n").collect();
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_sized(spec, bytes.as_bytes(), Some((24, 200)), 10_000, |_| {}).unwrap();
    let all: Vec<String> = (0..3000).map(wide).collect();
    t.resize(24, 30).unwrap();
    let got: Vec<String> = t.text(true).lines().map(str::to_string).collect();
    assert_eq!(got.len(), all.len(), "every line is kept");
    assert!(got == all, "every line is kept whole and in order");
    assert!(t.with_term(|t| t.grid().history_size()) <= 1000, "alacritty is back to its window");
}
