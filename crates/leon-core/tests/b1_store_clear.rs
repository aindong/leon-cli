//! Scrollback B1, criterion 5: a history clear (`CSI 3 J`, `ESC c`) clears the store too, and the
//! count moves past every number a client could keep.

use alacritty_terminal::grid::Dimensions;
use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

fn numbered(to: usize) -> String {
    (0..to).map(|i| format!("old-{i}\r\n")).collect()
}

#[test]
fn a_history_clear_empties_the_store() {
    let full = shown(numbered(5000).as_bytes());
    let before = full.lines(0, 0, &DARK);
    assert_eq!(before.history, 4977);
    assert!(full.with_term(|t| t.grid().history_size()) < before.history, "the store holds lines before the clear");
    for clear in ["\x1b[3J", "\x1bc"] {
        let t = shown(format!("{}{clear}after\r\n", numbered(5000)).as_bytes());
        let l = t.lines(0, 0, &DARK);
        assert!(l.history <= 1, "{clear:?}: nothing old is kept ({} lines)", l.history);
        assert!(l.scrolled - l.history > before.scrolled + 24, "{clear:?}: no old number is reused");
        for from in (0..before.scrolled + 24).step_by(1000) {
            let rows = t.lines(from, 1000, &DARK).rows;
            assert!(
                rows.iter().all(|r| !row_text(&r.spans).starts_with("old-")),
                "{clear:?}: an old line is read back"
            );
        }
        let text = t.text(true);
        assert!(
            !text.contains("old-100\n") && !text.contains("old-4000\n"),
            "{clear:?}: text after the clear: {text:.200}"
        );
    }
}
