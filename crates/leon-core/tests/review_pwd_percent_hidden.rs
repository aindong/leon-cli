//! Review proof: pwd_changed must not carry percent-encoded bidi/control characters.

use std::sync::{mpsc, Mutex};
use std::time::Duration;

use leon_core::terminal::{Notice, Spec, Terminal};

#[test]
fn pwd_event_carries_no_percent_encoded_hidden_chars() {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let script = r"printf '\033]7;file://h/x%%E2%%80%%AEy%%1Bz\007'";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let _t = Terminal::spawn(spec, move |n| tx.lock().unwrap().send(n).unwrap()).unwrap();
    let mut pwd = String::new();
    loop {
        match rx.recv_timeout(Duration::from_secs(10)).expect("no exit") {
            Notice::Pwd(p) => pwd = p,
            Notice::Exited(_) => break,
            _ => {}
        }
    }
    assert_eq!(pwd, "", "the whole pwd is dropped");
    let up = pwd.to_ascii_uppercase();
    assert!(
        !up.contains("%E2%80%AE") && !up.contains("%1B"),
        "percent-encoded hidden chars reached pwd_changed: {pwd:?}"
    );
}
