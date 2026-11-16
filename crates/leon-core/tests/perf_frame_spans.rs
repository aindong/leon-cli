//! A frame's spans for wide chars and combining marks followed by same-style text: pinned from the
//! code before `push_cell` stopped counting the chars of long spans.

use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

#[test]
fn wide_chars_and_marks_join_text_as_before() {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let shown = "\u{4e2d}abc\r\n\u{4e2d}\u{301}abc\r\ne\u{301}abc\r\nab\u{4e2d}cd\u{301}ef\r\nplain text run";
    let t = Terminal::spawn_showing(spec, shown.as_bytes(), |_| {}).unwrap();
    let f = t.frame(&DARK);
    let rows: Vec<Vec<(usize, usize, String)>> =
        f.rows[..5].iter().map(|r| r.iter().map(|s| (s.col, s.cells, s.text.clone())).collect()).collect();
    let want: Vec<Vec<(usize, usize, &str)>> = vec![
        vec![(0, 2, "\u{4e2d}"), (2, 3, "abc")],
        vec![(0, 5, "\u{4e2d}\u{301}abc")],
        vec![(0, 1, "e\u{301}"), (1, 3, "abc")],
        vec![(0, 2, "ab"), (2, 2, "\u{4e2d}"), (4, 1, "c"), (5, 1, "d\u{301}"), (6, 2, "ef")],
        vec![(0, 14, "plain text run")],
    ];
    let want: Vec<Vec<(usize, usize, String)>> =
        want.into_iter().map(|r| r.into_iter().map(|(c, n, s)| (c, n, s.to_string())).collect()).collect();
    assert_eq!(rows, want);
}
