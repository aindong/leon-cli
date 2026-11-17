//! App update criterion 4 (.agent/plans/2026-11-16-app-update-notice.md): with
//! `check_for_updates` off the app makes no request of its own, and Check for Updates still asks
//! when the user runs it. With the setting on, the check goes out once the settings are known.
//! The release server is a loopback listener that counts connections.

mod m10b_support;

use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m10b_support::*;

fn listener() -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let n = Arc::new(AtomicUsize::new(0));
    let count = n.clone();
    std::thread::spawn(move || {
        for stream in l.incoming().flatten() {
            count.fetch_add(1, Ordering::SeqCst);
            drop(stream); // no answer: the check fails, which is silent
        }
    });
    (url, n)
}

fn wait(cx: &mut VisualTestContext, secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(secs);
    while !done() && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    done()
}

fn updating(ws: &Entity<Workspace>, cx: &mut VisualTestContext) {
    ws.update(cx, |ws, cx| {
        ws.enable_updates(None, Arc::new(leon_update::Target::running));
        ws.sync_keymap(cx); // the settings arrive: the daily check may start
    });
}

#[gpui::test]
fn off_makes_no_request_and_check_for_updates_still_asks(cx: &mut TestAppContext) {
    let (url, asked) = listener();
    std::env::set_var("LEON_RELEASE_URL", &url);
    {
        let s = setup_with_config("leon.set(\"check_for_updates\", false)\n");
        let (ws, vcx) = open(cx, &s.app);
        updating(&ws, vcx);
        wait(vcx, 3, || false);
        assert_eq!(asked.load(Ordering::SeqCst), 0, "the setting is off: no request at all");
        act(&ws, vcx, "client.update.check");
        assert!(wait(vcx, 10, || asked.load(Ordering::SeqCst) >= 1), "Check for Updates asks even so");
        s.app.stop();
    }
    let before = asked.load(Ordering::SeqCst);
    let s = setup_with_config("");
    let (ws, vcx) = open(cx, &s.app);
    updating(&ws, vcx);
    assert!(wait(vcx, 10, || asked.load(Ordering::SeqCst) > before), "on by default: the daily check goes out");
    s.app.stop();
}
