//! What a terminal reports (.agent/plans/2026-11-14-m3-layout-events.md): OSC 7 in any chunking and
//! bounded, and the exit reported only after the last output's notices.

use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;

use leon_core::program_status::{Scanner, Tracker};
use leon_core::terminal::{Spec, Terminal};

#[test]
fn osc7_is_found_byte_by_byte_and_passed_on() {
    let input = b"a\x1b]7;file://h/tmp\x07b\x1b]7;file://h/x\x1b\\";
    let mut t = Tracker::default();
    let mut pass = Vec::new();
    let mut pwds = Vec::new();
    for b in input {
        let fed = t.feed(&[*b], |p| pass.extend_from_slice(p));
        pwds.extend(fed.pwd);
    }
    assert_eq!(pwds, [b"file://h/tmp".to_vec(), b"file://h/x".to_vec()]);
    assert_eq!(pass, input, "OSC 7 still reaches the emulator");
}

#[test]
fn an_oversized_osc7_is_dropped_and_the_scanner_recovers() {
    let mut s = Scanner::default();
    let long = format!("\x1b]7;file://h/{}\x07", "x".repeat(5000));
    s.feed(long.as_bytes(), |_| {});
    assert_eq!(s.take_pwd(), None);
    s.feed(b"\x1b]7;file://h/ok\x07", |_| {});
    assert_eq!(s.take_pwd(), Some(b"file://h/ok".to_vec()));
}

#[test]
fn the_exit_comes_after_the_last_output() {
    for _ in 0..5 {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);
        let spec = Spec {
            command: vec![
                "sh".into(),
                "-c".into(),
                r"head -c 300000 /dev/zero | tr '\0' x; printf '\033]2;bye\007\007'; exit 3".into(),
            ],
            cwd: None,
            env: vec![],
        };
        let _t = Terminal::spawn(spec, move |n| tx.lock().unwrap().send(format!("{n:?}")).unwrap()).unwrap();
        let mut got = Vec::new();
        while got.last().is_none_or(|n: &String| !n.starts_with("Exited")) {
            got.push(rx.recv_timeout(Duration::from_secs(10)).expect("no exit"));
        }
        assert_eq!(got, ["Title(\"bye\")", "Bell", "Exited(3)"]);
    }
}
