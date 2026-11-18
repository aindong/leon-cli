//! Scrollback B1, criterion 10: a full-screen program (the alternate screen) neither moves rows
//! into the store nor clears it: its output and its `CSI 3 J` touch only its own screen.

use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

fn wait(t: &Terminal, what: &str, pred: impl Fn(&Terminal) -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !pred(t) {
        assert!(Instant::now() < end, "never {what}: {:.300}", t.text(true));
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Every kept line's text, oldest first.
fn kept(t: &Terminal) -> Vec<String> {
    let l = t.lines(0, 0, &DARK);
    let oldest = l.scrolled - l.history;
    (oldest..l.scrolled)
        .step_by(1000)
        .flat_map(|from| t.lines(from, 1000.min(l.scrolled - from), &DARK).rows)
        .map(|r| row_text(&r.spans))
        .collect()
}

#[test]
fn the_alternate_screen_leaves_the_store_alone() {
    let script = "stty raw -echo; exec cat";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    t.write(b"ready\r\n").unwrap();
    wait(&t, "ready", |t| t.text(true).ends_with("ready"));
    let lines: String = (0..3000).map(|i| format!("kept {i}\r\n")).collect();
    t.write(lines.as_bytes()).unwrap();
    wait(&t, "printed the lines", |t| t.text(true).ends_with("kept 2999"));
    // a pause in output moves history past alacritty's window into the store
    wait(&t, "moved into the store", |t| t.with_term(|t| t.grid().history_size()) <= 1000);
    let before = kept(&t);
    assert!(before.len() > 2900, "the history is kept: {}", before.len());
    assert!(t.with_term(|t| t.grid().history_size()) < before.len(), "the store holds lines");

    let mut full_screen = b"\x1b[?1049h".to_vec();
    full_screen.extend("drawn\r\n".repeat(1000).bytes()); // 7 KB: more than one piece
    full_screen.extend(b"\x1b[3Jon the alternate screen");
    t.write(&full_screen).unwrap();
    wait(&t, "drew", |t| t.text(true).ends_with("on the alternate screen"));
    assert_eq!(t.lines(0, 0, &DARK).history, 0, "no history shows on the alternate screen");
    let text = t.text(false);
    assert!(!text.contains("kept 1\n"), "text is the alternate screen alone");

    t.write(b"\x1b[?1049lback\r\n").unwrap();
    wait(&t, "came back", |t| t.text(true).ends_with("back"));
    let after = kept(&t);
    assert_eq!(after[..before.len()], before[..], "every kept line is still kept, with its text");
}
