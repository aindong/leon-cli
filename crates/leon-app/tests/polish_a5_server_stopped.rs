//! Daily-driver A5: when This Mac's server stops from outside (`leon server stop`), the window says
//! so and offers to start it again or quit, instead of vanishing. Quit and End All Sessions, which
//! stops it on purpose, still just quits.

mod m10b_support;

use gpui::{Modifiers, TestAppContext};
use leon_app::view::QuitAndEndAll;
use m10b_support::*;
use serde_json::json;

#[gpui::test]
fn a_stopped_server_leaves_a_notice_with_start_and_quit(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    assert!(!drawn(cx, "server-stopped"));
    s.app.call("server.stop", json!({})).unwrap();
    s.app.stop();
    // a stopped server's process exits, closing its connections; the embedded one's go with it
    drop(s);
    assert!(shows(cx, |cx| drawn(cx, "server-stopped")), "the window says the server stopped");
    assert!(ws.read_with(cx, |ws, _| ws.server_stopped()));
    assert!(drawn(cx, "server-stopped-start"), "Start It Again");
    assert!(drawn(cx, "server-stopped-quit"), "Quit");
    assert!(!drawn(cx, "empty-state"), "not the no-session state");
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "keys act on nothing behind the notice");

    // Start It Again: this executable with its arguments (a test workspace records it, never spawns)
    let start = at(cx, "server-stopped-start").unwrap();
    cx.simulate_click(center(start), Modifiers::none());
    let argv = ws.read_with(cx, |ws, _| ws.relaunched()).expect("Start It Again ran the relaunch");
    let mut want = vec![std::env::current_exe().unwrap().display().to_string()];
    want.extend(std::env::args().skip(1));
    assert_eq!(argv, want);
}

#[gpui::test]
fn quit_and_end_all_still_quits_without_the_notice(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.dispatch_action(QuitAndEndAll);
    cx.run_until_parked();
    assert!(cx.has_pending_prompt(), "it asks first");
    cx.simulate_prompt_answer("End All Sessions");
    cx.run_until_parked();
    assert!(eventually(|| s.app.stopped()), "the server stopped");
    drop(s); // its connections close, as a stopped server's do
             // the poll sees the server gone; GPUI's test platform ignores quit(), so the window stays.
             // A negative needs a window of time: the poll wakes at once on the drop (a stop from outside
             // shows its notice within it, the test above), and this gives it a second and a half
    let end = std::time::Instant::now() + std::time::Duration::from_millis(1500);
    while std::time::Instant::now() < end && !ws.read_with(cx, |ws, _| ws.server_stopped()) {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!ws.read_with(cx, |ws, _| ws.server_stopped()), "ending on purpose shows no notice");
    assert!(!drawn(cx, "server-stopped"));
}
