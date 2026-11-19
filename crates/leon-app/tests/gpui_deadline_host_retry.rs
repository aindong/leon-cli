//! GPUI phase 5: a host whose connection failed tries again after its backoff, on a timer of its
//! own: nothing else wakes the window here. GPUI's clock moves with the real one (the hosts list
//! times its backoff on the real clock).
mod m10b_support;

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{px, size, TestAppContext};
use leon_app::view::{self, Workspace};
use m10b_support::*;

#[gpui::test]
fn a_failed_host_tries_again_after_its_backoff(cx: &mut TestAppContext) {
    let s = setup();
    cx.update(view::init);
    let app = s.app.clone();
    // no endpoint: each attempt fails at once, with no ssh
    let nowhere = leon_hosts::Host { label: "nowhere".into(), endpoints: Vec::new(), icon: None };
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(app.socket()).expect("the app attaches"));
        Workspace::with_hosts(remote, None, vec![nowhere], window, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    let attempts = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.host_attempts(1));
    assert!(shows(cx, |cx| attempts(cx) == 1), "the first attempt ran");
    let end = Instant::now() + Duration::from_secs(6); // the first backoff is 2 s
    while attempts(cx) < 2 && Instant::now() < end {
        let t = Instant::now();
        std::thread::sleep(Duration::from_millis(20));
        cx.executor().advance_clock(t.elapsed());
        cx.run_until_parked();
    }
    let n = attempts(cx);
    s.app.stop();
    assert!(n >= 2, "the host tried again after its backoff: {n} attempts");
}
