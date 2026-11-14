//! Review proof (correctness): a resize that pushes rows into the scrollback keeps every kept
//! line numbered (`scrolled >= history`).

use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

#[test]
fn a_resize_that_pushes_lines_keeps_scrolled_at_least_history() {
    let bytes: String = (0..30).map(|i| format!("{i}\r\n")).collect();
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_showing(spec, bytes.as_bytes(), |_| {}).unwrap();
    t.resize(12, 80).unwrap(); // cursor on row 23: 12 lines go into the scrollback
    let l = t.lines(0, 0, &DARK);
    assert!(l.scrolled >= l.history, "scrolled {} < history {}", l.scrolled, l.history);
}
