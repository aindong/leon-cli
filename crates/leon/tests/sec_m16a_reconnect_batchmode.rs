//! M16a security review: once `leon attach -H` owns the terminal (raw mode, stderr on /dev/null),
//! a reconnect must not ask for a password on the tty: the keystrokes would be split between ssh
//! and the TUI, and part of a password could be typed into the remote shell. So those ssh runs get
//! BatchMode, and fail instead of prompting (the retry loop carries on).

mod support;

use support::host::{attach_then_stty, Host};
use support::remote::Remote;

#[test]
fn reconnects_under_the_tui_never_prompt() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "bm"]);
    let vars = r.vars();
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let h = Host::spawn(&r.local, &attach_then_stty("bm -H ssh://box"), &vars);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("bm"));
    let before = r.ssh_calls().len();
    assert!(r.ssh_calls().iter().all(|c| !c.contains(&"BatchMode=yes".to_string())), "the first logins may prompt");
    assert!(r.kill_transports() >= 3);
    r.until("attach to reconnect", || r.ssh_calls().len() >= before + 3);
    for c in &r.ssh_calls()[before..] {
        assert!(c.iter().any(|a| a == "BatchMode=yes"), "a reconnect can prompt on the tty: {c:?}");
    }
    h.send(b"\x02d");
}
