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

/// A program that prints what it is sent, with `prefix-0..3000` printed and moved into the store.
fn live_with_a_full_store(prefix: &str) -> Terminal {
    let spec =
        Spec { command: vec!["sh".into(), "-c".into(), "stty raw -echo; exec cat".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    t.write(b"ready\r\n").unwrap();
    until(&t, "ready", |t| t.text(true).ends_with("ready"));
    t.write((0..3000).map(|i| format!("{prefix}-{i}\r\n")).collect::<String>().as_bytes()).unwrap();
    until(&t, "printed", |t| t.text(true).ends_with(&format!("{prefix}-2999")));
    until(&t, "moved into the store", |t| t.with_term(|t| t.grid().history_size()) <= 1000);
    assert!(t.lines(0, 0, &DARK).history > 2900, "the store holds the lines");
    t
}

fn until(t: &Terminal, what: &str, pred: impl Fn(&Terminal) -> bool) {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !pred(t) {
        assert!(std::time::Instant::now() < end, "never {what}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn a_clear_empties_a_full_store() {
    for clear in ["\x1b[3J", "\x1bc"] {
        let t = live_with_a_full_store("old");
        t.write(format!("{clear}after\r\n").as_bytes()).unwrap();
        until(&t, "cleared", |t| t.text(true).ends_with("after"));
        assert!(t.lines(0, 0, &DARK).history <= 1, "{clear:?}: nothing old is kept");
        assert!(!t.text(true).contains("old-100\n"), "{clear:?}: the store outlived the clear");
    }
}

#[test]
fn a_clear_on_the_alternate_screen_keeps_a_full_store() {
    let t = live_with_a_full_store("kept");
    t.write(b"\x1b[?1049hdrawn\x1b[3J").unwrap();
    until(&t, "drawn", |t| t.text(true).ends_with("drawn"));
    t.write(b"\x1b[?1049lback\r\n").unwrap();
    until(&t, "back", |t| t.text(true).ends_with("back"));
    let text = t.text(true);
    assert!((0..3000).all(|i| text.contains(&format!("kept-{i}\n"))), "the alternate screen's clear is its own");
}
