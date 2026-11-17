//! Settings › Server: This Mac's server as `server.status` reports it, and Stop and Restart, which
//! ask first with what happens to the programs, and do it, on a private test server.
mod m9_support;
mod settings_support;
use std::rc::Rc;

use gpui::{TestAppContext, VisualTestContext};
use leon_app::settings_window::{running_for, stop_warning, SettingsWindow};
use serde_json::json;
use settings_support::*;

#[test]
fn the_warnings_say_what_stop_does() {
    let (ask, detail) = stop_warning(false, true);
    assert_eq!(ask, "Stop the leon server?");
    assert!(detail.starts_with("Every program in every session ends now."), "{detail}");
    assert!(detail.contains("sessions are saved") && detail.contains("come back"), "{detail}");
    let (_, detail) = stop_warning(false, false);
    assert!(detail.contains("doesn't save its sessions") && !detail.contains("come back"), "{detail}");
    let (ask, detail) = stop_warning(true, true);
    assert_eq!(ask, "Restart the leon server?");
    assert!(detail.contains("fresh shell") && detail.contains("resume command"), "{detail}");
    assert_eq!(
        (running_for(100, 130), running_for(0, 7500), running_for(0, 2 * 86_400 + 3600)),
        ("less than a minute".to_string(), "2 h 5 min".to_string(), "2 days 1 h".to_string())
    );
}

#[gpui::test]
fn the_page_shows_this_macs_server(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    o.s.app.call("session.create", json!({})).unwrap();
    click(&mut cx, "section-server");
    let info = |cx: &mut VisualTestContext| o.sw.read_with(cx, |sw, _| sw.server_info()).unwrap();
    assert!(shows(&mut cx, |cx| matches!(info(cx), Some(Ok(i)) if i.blocks == 1)), "{:?}", info(&mut cx));
    let i = info(&mut cx).unwrap().unwrap();
    let status = o.s.app.call("server.status", json!({})).unwrap();
    assert_eq!((i.sessions, i.blocks, i.version.as_str()), (1, 1, env!("CARGO_PKG_VERSION")));
    assert_eq!(i.started_at, status["started_at"].as_u64().unwrap());
    assert_eq!(i.socket, o.s.app.socket().display().to_string());
    assert!(!i.saves && i.log.is_none(), "an embedded server saves nothing and logs nowhere: {i:?}");
    let seen = ["server-running", "server-stop", "tailscale-section"].map(|s| drawn(&mut cx, s));
    let unseen = ["server-log", "server-restart"].map(|s| drawn(&mut cx, s));
    o.s.app.stop();
    assert_eq!(seen, [true; 3], "running, Stop, and Tailscale's place");
    assert_eq!(unseen, [false; 2], "no log to reveal; no main window to start again");
}

#[gpui::test]
fn stop_asks_first_then_stops(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    click(&mut cx, "section-server");
    assert!(shows(&mut cx, |cx| drawn(cx, "server-stop")));
    click(&mut cx, "server-stop");
    let (ask, detail) = stop_warning(false, false);
    assert_eq!(cx.pending_prompt(), Some((ask.clone(), detail.clone())), "it asks first, saying what happens");
    cx.simulate_prompt_answer("Cancel");
    // a stop would end the server's loop within a second or so: watch that long
    for _ in 0..50 {
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(!o.s.app.stopped(), "Cancel leaves it running");

    click(&mut cx, "server-stop");
    assert_eq!(cx.pending_prompt(), Some((ask, detail)));
    cx.simulate_prompt_answer("Stop Server");
    let gone = shows(&mut cx, |cx| matches!(o.sw.read_with(cx, |sw, _| sw.server_info()).unwrap(), Some(Err(_))));
    let stopped = m9_support::eventually(|| o.s.app.stopped());
    o.s.app.stop();
    assert!(gone, "the page says it isn't running");
    assert!(stopped, "the server stopped");
}

#[gpui::test]
fn restart_asks_first_then_stops_and_starts_leon_again(cx: &mut TestAppContext) {
    let s = m9_support::setup_with_config("");
    let config = s.dir.path().join("init.lua");
    let (ws, wcx) = m9_support::open(cx, &s.app);
    let remote = Rc::new(leon_app::remote::Remote::connect(s.app.socket()).unwrap());
    let sw = wcx.update(|_, cx| SettingsWindow::open(config, remote, cx)).unwrap();
    let mut cx = VisualTestContext::from_window(sw.into(), wcx);
    cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(2400.)));
    click(&mut cx, "section-server");
    assert!(shows(&mut cx, |cx| drawn(cx, "server-restart")), "the main window is open: Restart");
    click(&mut cx, "server-restart");
    assert_eq!(cx.pending_prompt(), Some(stop_warning(true, false)));
    assert_eq!(ws.read_with(&cx, |ws, _| ws.relaunched()), None);
    cx.simulate_prompt_answer("Restart");
    assert!(shows(&mut cx, |_| s.app.stopped()), "the server stopped");
    cx.run_until_parked();
    let early = ws.read_with(&cx, |ws, _| ws.relaunched());
    // a stopped server's connections close with its process; the embedded one's when it goes
    s.app.stop();
    drop(s);
    let again = shows(&mut cx, |cx| ws.read_with(cx, |ws, _| ws.relaunched()).is_some());
    assert_eq!(early, None, "not while the main window still had the old server");
    assert!(again, "leon started again once the main window saw the server gone");
    assert!(ws.read_with(&cx, |ws, _| ws.server_stopped()));
}

#[gpui::test]
fn search_finds_the_server(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    cx.simulate_input("restart");
    let got = o.sw.read_with(&cx, |sw, _| sw.shown()).unwrap();
    o.s.app.stop();
    assert!(got.contains(&"server".to_string()), "{got:?}");
}
