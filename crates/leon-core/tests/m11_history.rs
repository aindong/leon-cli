//! M11 criterion 1: absolute line numbers and history ranges (`Terminal::lines`).

use std::time::{Duration, Instant};

use leon_core::screen::row_text;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

/// A terminal that shows `bytes` as if its program had printed them, then sleeps.
fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

fn numbered(from: usize, to: usize) -> String {
    (from..to).map(|i| format!("{i}\r\n")).collect()
}

/// The text of absolute line `line`, or None when it isn't kept.
fn line(t: &Terminal, line: usize) -> Option<String> {
    let l = t.lines(line, 1, &DARK);
    l.rows.iter().find(|r| r.line == line).map(|r| row_text(&r.spans))
}

#[test]
fn printed_lines_keep_their_numbers() {
    let t = shown(numbered(0, 30000).as_bytes());
    let l = t.lines(0, 0, &DARK);
    // 24 rows: the last holds the cursor, so the screen's top row is line 29977
    assert_eq!((l.scrolled, l.history), (29977, 10000));
    for n in [19977, 20000, 25000, 29976, 29977, 29999] {
        assert_eq!(line(&t, n).as_deref(), Some(n.to_string().as_str()), "line {n}");
    }
    assert_eq!(line(&t, 19976), None, "older than the scrollback keeps");
    let range = t.lines(29970, 20, &DARK);
    let texts: Vec<String> = range.rows.iter().map(|r| row_text(&r.spans)).collect();
    let want: Vec<String> = (29970..29990).map(|n| n.to_string()).collect();
    assert_eq!(texts, want, "a range across history and screen");
    assert!(t.lines(0, 5000, &DARK).rows.len() <= 1000, "a range is capped");
}

#[test]
fn numbers_hold_across_many_batches_of_real_output() {
    let script = "stty -echo; seq 0 29999; read x; seq 30000 30099; sleep 60";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    let wait = |what: &str| {
        let end = Instant::now() + Duration::from_secs(20);
        while !t.text(true).ends_with(what) {
            assert!(Instant::now() < end, "never printed {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait("29999");
    assert_eq!(line(&t, 25000).as_deref(), Some("25000"));
    t.write(b"\n").unwrap();
    wait("30099");
    for n in [25000, 29999, 30000, 30050, 30099] {
        assert_eq!(line(&t, n).as_deref(), Some(n.to_string().as_str()), "line {n} after more output");
    }
}

#[test]
fn an_alt_screen_program_moves_nothing() {
    let mut bytes = numbered(0, 50);
    let before = shown(bytes.as_bytes()).lines(0, 0, &DARK).scrolled;
    bytes.push_str("\x1b[?1049h");
    bytes.push_str(&"drawn\r\n".repeat(100));
    bytes.push_str("\x1b[?1049l");
    let t = shown(bytes.as_bytes());
    assert_eq!(t.lines(0, 0, &DARK).scrolled, before, "the alt screen has no scrollback");
    assert_eq!(line(&t, 10).as_deref(), Some("10"));
}

/// The absolute number of the kept line whose text is `text`.
fn number_of(t: &Terminal, text: &str) -> Option<usize> {
    let l = t.lines(0, 0, &DARK);
    let oldest = l.scrolled - l.history;
    let rows = (oldest..l.scrolled + 24).step_by(1000).flat_map(|from| t.lines(from, 1000, &DARK).rows);
    rows.into_iter().find(|r| row_text(&r.spans) == text).map(|r| r.line)
}

#[test]
fn a_history_clear_then_more_output_still_numbers_right() {
    let first = numbered(0, 100);
    let old = shown(first.as_bytes());
    let old_50 = number_of(&old, "50").unwrap();
    let t = shown(format!("{first}\x1b[3J{}", numbered(100, 200)).as_bytes());
    let l = t.lines(0, 0, &DARK);
    assert!(l.history < 177, "the clear dropped the old scrollback: {}", l.history);
    let (a, b) = (number_of(&t, "150").unwrap(), number_of(&t, "199").unwrap());
    assert_eq!(b - a, 49, "the new lines are numbered in order");
    assert_ne!(line(&t, old_50).as_deref(), Some("150"), "no old number lands on a new line by chance");
    assert!(line(&t, old_50).is_none() || line(&t, old_50).as_deref() == Some("50"));
}

#[test]
fn a_flood_past_the_scrollback_leaves_no_old_number_on_a_new_line() {
    let mut bytes = numbered(0, 1000);
    let t = shown(bytes.as_bytes());
    let last = t.lines(0, 0, &DARK).scrolled - 1;
    assert_eq!(line(&t, last).as_deref(), Some(last.to_string().as_str()));
    // 4 KiB of "scroll up 24": far more lines than the scrollback holds, in one slice
    bytes.push_str(&"\x1b[24S".repeat(4096 / 5));
    bytes.push_str("end\r\n");
    let t = shown(bytes.as_bytes());
    let l = t.lines(0, 0, &DARK);
    assert!(last < l.scrolled - l.history, "line {last} is gone, not renamed: {} {}", l.scrolled, l.history);
    assert_eq!(line(&t, last), None);
}
