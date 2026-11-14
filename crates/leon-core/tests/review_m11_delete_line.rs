//! Review proof (correctness): scrolls that don't enter the scrollback (delete-line, a top
//! margin) renumber nothing.

use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

fn shown(bytes: &str) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes.as_bytes(), |_| {}).unwrap()
}

fn text(t: &Terminal, line: usize) -> Option<String> {
    t.lines(line, 1, &DARK).rows.iter().find(|r| r.line == line).map(|r| row_text(&r.spans))
}

#[test]
fn a_delete_line_renumbers_nothing() {
    let mut bytes: String = (0..1000).map(|i| format!("{i}\r\n")).collect();
    bytes.push_str("\x1b[5;1H\x1b[M"); // delete screen row 5: nothing enters the scrollback
    let t = shown(&bytes);
    assert_eq!(text(&t, 500).as_deref(), Some("500"));
}

#[test]
fn a_top_margin_renumbers_nothing_with_a_full_scrollback() {
    let mut bytes: String = (0..12000).map(|i| format!("{i}\r\n")).collect();
    bytes.push_str("\x1b[3;24r\x1b[24;1H"); // rows 3–24 scroll; rows 1–2 stay
    bytes.push_str(&"x\r\n".repeat(50));
    bytes.push_str("\x1b[r");
    let t = shown(&bytes);
    assert_eq!(text(&t, 5000).as_deref(), Some("5000"));
}
