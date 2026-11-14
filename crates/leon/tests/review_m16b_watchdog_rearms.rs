//! M16b review: the watchdog must keep watching after a ping was answered. A host link is quiet
//! within seconds (nothing watched on it), so its first ping comes early; a stall after that must
//! still be found and resumed in about 15 s, not ssh's 45.

#[path = "support/app.rs"]
mod app;
mod support;

use std::time::{Duration, Instant};

use app::*;
use gpui::TestAppContext;

#[gpui::test]
fn the_watchdog_still_notices_after_an_answered_ping(cx: &mut TestAppContext) {
    let h = hosted();
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    wait_connected(&ws, cx, "devbox");
    let remote = ws.read_with(cx, |ws, _| ws.host_remote("devbox")).unwrap();
    // the scenario is the quiet spell: one ping goes out at about 5 s and is answered
    std::thread::sleep(Duration::from_secs(12));
    let resumed = remote.resumes();
    assert!(signal_transports(&h.r, libc::SIGSTOP) >= 3);
    let end = Instant::now() + Duration::from_secs(25);
    while remote.resumes() == resumed && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    let ok = remote.resumes() > resumed;
    signal_transports(&h.r, libc::SIGCONT);
    assert!(ok, "watchdog disarmed after its first answered ping");
    h.r.kill_transports();
    h.local.stop();
}
