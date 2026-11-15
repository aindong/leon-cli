//! F12 follow-up: keys held behind attach's probe must still reach the pane before a paste typed
//! after them. The transports are stopped (SIGSTOP) so the probe is still out when the paste comes.

mod support;

use std::time::Duration;

use support::host::{attach_then_stty, Host};
use support::remote::Remote;

#[test]
fn a_paste_lands_after_the_keys_typed_before_it() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "new", "r1"]);
    let b = r.ok(&["-H", "ssh://box", "-s", "r1", "run", "--", "sh", "-c", "echo hello-remote; exec sh"]);
    r.until("the remote block's first line", || {
        r.ok(&["-H", "ssh://box", "capture", "-b", &b]).contains("hello-remote")
    });
    let vars = r.vars();
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let h = Host::spawn(&r.local, &attach_then_stty("r1 -H ssh://box"), &vars);
    h.wait_text("hello-remote");

    let stopped = r.signal_transports(libc::SIGSTOP);
    assert!(stopped >= 3, "attach runs over its transports");
    // the first key sends a probe; the rest of the keys wait behind it
    h.send(b"echo typed");
    // then the user pastes (bracketed paste: attach enables it)
    h.send(b"\x1b[200~-then-pasted-$((6*7))\x1b[201~");
    // the scenario's input: a short hiccup, well under the 5 s call timeout and the watchdog
    std::thread::sleep(Duration::from_millis(500));
    r.signal_transports(libc::SIGCONT);
    std::thread::sleep(Duration::from_millis(300));
    h.send(b"\r");
    // in order the shell runs `echo typed-then-pasted-$((6*7))`; out of order it runs
    // `-then-pasted-42echo typed`, which isn't a command
    h.wait_text("typed-then-pasted-42");
}
