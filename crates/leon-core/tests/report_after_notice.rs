//! A terminal's reported title, pwd and foreground change no earlier than their notice is handled.
//! A server handles it under its state lock (`State::notice`), storing the value and sending the
//! event together, so a snapshot (taken under the same lock) never shows a value whose event is
//! still to come: an attach between the two would get the change twice.

use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use leon_core::terminal::{Notice, Spec, Terminal};

/// Start `command` with a notify that records, for each title, pwd and foreground notice, what the
/// terminal reported at that moment; the receiver gets (notice, reported then).
fn spawn(command: &[&str]) -> (Arc<Terminal>, mpsc::Receiver<(String, Option<String>)>) {
    let me: Arc<OnceLock<Arc<Terminal>>> = Arc::default();
    let (tx, rx) = mpsc::channel();
    let spec = Spec { command: command.iter().map(|s| s.to_string()).collect(), cwd: None, env: vec![] };
    let mine = me.clone();
    let t = Terminal::spawn(spec, move |n| {
        // the program can report before spawn returns: wait for `me`, as the server's state lock does
        let t = mine.wait();
        let seen = match n {
            Notice::Title(v) => (format!("title {v}"), t.title()),
            Notice::Pwd(v) => (format!("pwd {v}"), t.cwd().map(|p| p.display().to_string())),
            Notice::Foreground(v) => (format!("foreground {v:?}"), t.reported_foreground()),
            _ => return,
        };
        let _ = tx.send(seen);
    })
    .unwrap();
    let t = Arc::new(t);
    me.set(t.clone()).ok();
    (t, rx)
}

#[test]
fn the_foreground_is_reported_once_its_notice_is_handled() {
    let (t, seen) = spawn(&["sleep", "30"]);
    let end = Instant::now() + Duration::from_secs(10);
    while t.foreground().as_deref() != Some("sleep") && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    t.check_foreground();
    let (notice, then) = seen.recv_timeout(Duration::from_secs(10)).expect("a foreground notice");
    assert_eq!(notice, "foreground Some(\"sleep\")");
    assert_eq!(then, None, "while its notice is handled, the reported foreground is still the old one");
    assert_eq!(t.reported_foreground().as_deref(), Some("sleep"), "and the new one after");
}

#[test]
fn the_title_is_reported_once_its_notice_is_handled() {
    let (t, seen) = spawn(&["sh", "-c", r"printf '\033]2;fresh\007'; sleep 30"]);
    let (notice, then) = seen.recv_timeout(Duration::from_secs(10)).expect("a title notice");
    assert_eq!(notice, "title fresh");
    assert_eq!(then, None, "while its notice is handled, the reported title is still the old one");
    // the reader stores it once its notify returned, and the notice reached us from inside that notify
    let end = Instant::now() + Duration::from_secs(10);
    while t.title().as_deref() != Some("fresh") && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(t.title().as_deref(), Some("fresh"), "and the new one after");
}
