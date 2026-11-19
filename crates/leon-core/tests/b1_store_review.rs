//! Scrollback B1, from the review: output that pushes many rows for few bytes loses no line on
//! its way into the store, and a history clear clears the store whatever comes with it.

use std::time::{Duration, Instant};

use alacritty_terminal::grid::Dimensions;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

fn shown(bytes: &[u8]) -> Terminal {
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    Terminal::spawn_showing(spec, bytes, |_| {}).unwrap()
}

/// The numbered lines `prefix-0..n` missing from the terminal's text.
fn lost(t: &Terminal, prefix: &str, n: usize) -> Vec<usize> {
    let text = t.text(true);
    let kept: std::collections::HashSet<&str> = text.lines().map(str::trim_end).collect();
    (0..n).filter(|i| !kept.contains(format!("{prefix}-{i}").as_str())).collect()
}

#[test]
fn a_synchronized_update_loses_no_line() {
    // vte holds the update and applies it at once: 3,000 rows pushed by one piece
    let mut s = String::from("\x1b[?2026h");
    s += &(0..3000).map(|i| format!("sync-{i}\r\n")).collect::<String>();
    s += "\x1b[?2026l";
    assert_eq!(lost(&shown(s.as_bytes()), "sync", 3000), Vec::<usize>::new());
}

#[test]
fn cursor_forward_wraps_lose_no_line() {
    // `CSI 99 C` puts the cursor on the last column, `a` fills it, `b` wraps: a row per 7 bytes
    let mut s: String = (0..2000).map(|n| format!("line-{n}\r\n")).collect();
    s += &"\x1b[99Cab".repeat(1200);
    s += "end\r\n";
    assert_eq!(lost(&shown(s.as_bytes()), "line", 2000), Vec::<usize>::new());
}

#[test]
fn screen_clears_lose_no_line() {
    // each `CSI 2 J` pushes the whole screen into the history
    let s: String = (0..100).map(|i| format!("\x1b[H\x1b[2J\x1b[24;1Hscreen-{i}")).collect();
    assert_eq!(lost(&shown(s.as_bytes()), "screen", 100), Vec::<usize>::new());
}

#[test]
fn a_history_clear_with_padded_parameters_empties_the_store() {
    let zeros = format!("\x1b[{}3J", "0".repeat(200));
    for clear in ["\x1b[03J", "\x1b[3;1J", "\x1b[003:0J", zeros.as_str()] {
        let t = live();
        t.write((0..3000).map(|i| format!("secret-{i}\r\n")).collect::<String>().as_bytes()).unwrap();
        until(&t, "printed", |t| t.text(true).ends_with("secret-2999"));
        until(&t, "moved into the store", |t| t.with_term(|t| t.grid().history_size()) <= 1000);
        t.write(format!("\x1b[H\x1b[2J{clear}after\r\n").as_bytes()).unwrap();
        until(&t, "cleared", |t| t.text(true).ends_with("after"));
        assert!(!t.text(true).contains("secret-"), "cleared history survived {clear:?}");
    }
}

#[test]
fn a_clear_then_the_alternate_screen_in_one_read_empties_the_store() {
    for pad in 0..6 {
        let mut out: String = "p".repeat(pad) + "\r\n";
        out += &(0..3000).map(|i| format!("secret-{i}\r\n")).collect::<String>();
        out += "\x1b[H\x1b[2J\x1b[3J\x1b[?1049h"; // `clear`, then a full-screen program
        out += &"x".repeat(8000);
        out += "\x1b[?1049lback\r\n";
        let t = shown(out.as_bytes());
        assert!(!t.text(true).contains("secret-"), "cleared history survived (pad {pad})");
    }
}

#[test]
fn a_clear_on_the_alternate_screen_keeps_the_store() {
    let mut out: String = (0..3000).map(|i| format!("kept-{i}\r\n")).collect();
    out += "\x1b[?1049hdrawn\x1b[3J\x1b[?1049lback\r\n";
    let t = shown(out.as_bytes());
    assert_eq!(lost(&t, "kept", 3000), Vec::<usize>::new(), "the alternate screen's clear is its own");
}

#[test]
fn a_clear_just_before_a_live_alternate_screen_empties_the_store() {
    let spec = Spec {
        command: vec!["sh".into(), "-c".into(), "stty raw -echo; printf 'ready\\r\\n'; exec cat".into()],
        cwd: None,
        env: vec![],
    };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    let wait = |what: &str, pred: &dyn Fn(&Terminal) -> bool| {
        let end = Instant::now() + Duration::from_secs(20);
        while !pred(&t) {
            assert!(Instant::now() < end, "never {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait("ready", &|t| t.text(true).ends_with("ready"));
    t.write((0..3000).map(|i| format!("old-{i}\r\n")).collect::<String>().as_bytes()).unwrap();
    wait("printed", &|t| t.text(true).ends_with("old-2999"));
    wait("settled", &|t| t.with_term(|t| t.grid().history_size()) <= 1000);
    t.write(b"\x1b[3J\x1b[?1049h").unwrap();
    wait("on the alternate screen", &|t| t.lines(0, 0, &DARK).history == 0 && t.text(true).is_empty());
    t.write(b"\x1b[?1049lback\r\n").unwrap();
    wait("back", &|t| t.text(true).ends_with("back"));
    assert!(!t.text(true).contains("old-100\n"), "the store outlived CSI 3 J");
}

/// A program that prints what it is sent, once its echo is off.
fn live() -> Terminal {
    let spec = Spec {
        command: vec!["sh".into(), "-c".into(), "stty raw -echo; printf 'ready\\r\\n'; exec cat".into()],
        cwd: None,
        env: vec![],
    };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    until(&t, "ready", |t| t.text(true).ends_with("ready"));
    t
}

fn until(t: &Terminal, what: &str, pred: impl Fn(&Terminal) -> bool) {
    let end = Instant::now() + Duration::from_secs(20);
    while !pred(t) {
        assert!(Instant::now() < end, "never {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_cap_raised_on_a_lowered_pane_keeps_more_again() {
    let t = live();
    t.set_scrollback(500);
    t.write((0..3000).map(|i| format!("first-{i}\r\n")).collect::<String>().as_bytes()).unwrap();
    until(&t, "printed", |t| t.text(true).ends_with("first-2999"));
    until(&t, "capped", |t| t.lines(0, 0, &DARK).history == 500);
    t.set_scrollback(20_000);
    t.write((0..3000).map(|i| format!("then-{i}\r\n")).collect::<String>().as_bytes()).unwrap();
    until(&t, "printed more", |t| t.text(true).ends_with("then-2999"));
    assert_eq!(lost(&t, "then", 3000), Vec::<usize>::new(), "the raised cap keeps every new line");
}

#[test]
fn what_a_program_printed_as_it_exited_moves_into_the_store() {
    let spec = Spec { command: vec!["sh".into(), "-c".into(), "seq 1 3000".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    until(&t, "moved into the store", |t| {
        t.text(true).ends_with("3000") && t.with_term(|t| t.grid().history_size()) <= 1000
    });
    assert!(t.lines(0, 0, &DARK).history > 2900, "all of it is kept");
}

#[test]
fn a_clear_held_in_a_synchronized_update_is_decided_on_its_own_screen() {
    let t = live();
    t.write((0..3000).map(|i| format!("kept-{i}\r\n")).collect::<String>().as_bytes()).unwrap();
    until(&t, "printed", |t| t.text(true).ends_with("kept-2999"));
    until(&t, "moved into the store", |t| t.with_term(|t| t.grid().history_size()) <= 1000);
    // vte holds the switch and the clear back until the update ends: the clear is the alternate
    // screen's own
    t.write(b"\x1b[?2026h\x1b[?1049h\x1b[3J\x1b[?2026ldrawn").unwrap();
    until(&t, "drawn", |t| t.text(true).ends_with("drawn"));
    t.write(b"\x1b[?1049lback\r\n").unwrap();
    until(&t, "back", |t| t.text(true).ends_with("back"));
    assert_eq!(lost(&t, "kept", 3000), Vec::<usize>::new(), "the primary screen's history is kept");
}

#[test]
fn a_linked_blank_keeps_its_link_in_the_store() {
    let mut s = String::from("\x1b]8;;https://example.com/padded\x1b\\label     \x1b]8;;\x1b\\\r\n");
    s += &(0..3000).map(|i| format!("after-{i}\r\n")).collect::<String>();
    let t = shown(s.as_bytes());
    let oldest = t.scrolled() - t.lines(0, 0, &DARK).history;
    let at = |col| leon_core::search::Pos { line: oldest, col };
    assert!(t.with_term(|t| t.grid().history_size()) <= 1000, "the line is in the store");
    for col in [0, 4, 5, 9] {
        let link = t.link_at(at(col)).unwrap_or_else(|| panic!("col {col}: no link"));
        assert_eq!(
            (link.uri.as_str(), link.start.col, link.end.col),
            ("https://example.com/padded", 0, 9),
            "col {col}"
        );
    }
}
