//! Review proof: title and pwd events carry no bidi overrides or invisible characters (spec §5
//! security rule, applied to every program-supplied text the server reports).

use std::sync::{mpsc, Mutex};
use std::time::Duration;

use leon_core::terminal::{Notice, Spec, Terminal};

#[test]
fn title_and_pwd_events_carry_no_bidi_or_invisible_chars() {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let script = r"printf '\033]2;a\342\200\256b\342\200\213c\007\033]7;file://h/x\342\200\256y\007'";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let _t = Terminal::spawn(spec, move |n| tx.lock().unwrap().send(n).unwrap()).unwrap();
    let (mut title, mut pwd) = (String::new(), String::new());
    loop {
        match rx.recv_timeout(Duration::from_secs(10)).expect("no exit") {
            Notice::Title(t) => title = t,
            Notice::Pwd(p) => pwd = p,
            Notice::Exited(_) => break,
            _ => {}
        }
    }
    let bad = |s: &str| s.contains(['\u{202e}', '\u{200b}']);
    assert!(!bad(&title) && !bad(&pwd), "bidi/invisible chars reached title_changed/pwd_changed: {title:?} {pwd:?}");
    assert_eq!(
        (title.as_str(), pwd.as_str()),
        ("abc", ""),
        "a title is stripped; a pwd with hidden characters is dropped"
    );
}
