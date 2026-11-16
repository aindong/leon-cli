//! M16a correctness review: `leon events -H` gives up within its hold (60 s; 5 s here, through
//! `LEON_LINK_HOLD_SECS`) even when a reconnect's ssh hangs (a blackholed network, a prompt nobody
//! answers) for longer than the test waits: each attempt has a deadline.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

use support::remote::Remote;

#[test]
fn events_reconnect_gives_up_when_ssh_hangs() {
    let r = Remote::new();
    let hang = r.fake.path().join("ssh-hang");
    std::fs::write(
        &hang,
        "#!/bin/sh\nif [ -e \"$FAKE_SSH_DIR/hang\" ]; then echo $$ >> \"$FAKE_SSH_DIR/hung\"; exec sleep 90; fi\n\
         exec \"$FAKE_SSH_DIR/ssh\" \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&hang, std::fs::Permissions::from_mode(0o700)).unwrap();
    let mut cmd = r.cmd();
    cmd.env("LEON_SSH", &hang).env("LEON_LINK_HOLD_SECS", "5");
    let mut ev = support::stream(cmd, &["-H", "ssh://me@box"]);
    std::fs::write(r.fake.path().join("hang"), "").unwrap();
    assert!(r.kill_transports() > 0, "no transport to kill");
    // the hold and 20 s: an attempt without a deadline waits out the ssh's 90 s
    let end = Instant::now() + Duration::from_secs(25);
    let code = loop {
        if let Some(st) = ev.child.try_wait().unwrap() {
            break st.code();
        }
        if Instant::now() > end {
            break None;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let hung = std::fs::read_to_string(r.fake.path().join("hung")).unwrap_or_default();
    for pid in hung.lines().filter_map(|p| p.trim().parse::<i32>().ok()) {
        // SAFETY: a signal to a `sleep` this test's fake ssh became
        unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    assert!(code.is_some(), "leon events -H still reconnecting 25 s after the link dropped");
    assert_eq!(code, Some(4), "gives up as unreachable");
}
