//! M10 criteria 1–5 (.agent/plans/2026-11-14-m10-app-attaches.md): the app is a client of a real
//! `leon server` process. Quitting it leaves every program running, opening it again shows the
//! same layout and live screens, and only "Quit and End All Sessions" (or `leon server stop`)
//! ends them. The window is GPUI's headless test window; the server is the real binary.

mod support;

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{px, size, Entity, TestAppContext, VisualTestContext};
use leon_app::remote::Remote;
use leon_app::view::{self, Quit, QuitAndEndAll, Workspace};
use serde_json::Value;
use support::Env;

/// Open the app on `env`'s server the way `leon app` does: its keys, menus and quit hooks, then
/// the window.
fn open<'a>(cx: &'a mut TestAppContext, env: &Env) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    let remote = Rc::new(Remote::connect(&env.socket()).expect("the app attaches"));
    cx.update(view::install);
    let (ws, vcx) = cx.add_window_view(move |window, cx| Workspace::new(remote, window, cx));
    vcx.simulate_resize(size(px(1400.), px(900.)));
    vcx.run_until_parked();
    (ws, vcx)
}

/// Quit the app through its own path: the Quit action (cmd+q), then GPUI's shutdown, which runs
/// every `on_app_quit` hook and closes the windows. The workspace and its connections go with it.
fn quit(ws: Entity<Workspace>, cx: &mut VisualTestContext) {
    cx.dispatch_action(Quit);
    cx.run_until_parked();
    drop(ws);
    cx.cx.update(|cx| cx.shutdown()); // app-wide: the window goes with it
    cx.cx.run_until_parked();
}

/// Poll `f` (letting the window run between tries) for up to 10 s until it returns Some.
fn eventually<T>(cx: &mut VisualTestContext, what: &str, mut f: impl FnMut(&mut VisualTestContext) -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        cx.run_until_parked();
        if let Some(v) = f(cx) {
            return v;
        }
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The highest `tick-N` in `text`.
fn last_tick(text: &str) -> Option<u64> {
    text.split_whitespace().filter_map(|w| w.strip_prefix("tick-")?.parse().ok()).max()
}

/// What the app knows of the selected session's tabs, in order: per window its id, label,
/// layout tree, focus and zoom, and which one is active; as JSON.
fn shown(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Value {
    let (windows, active) = ws.read_with(cx, |ws, _| ws.windows()).expect("a selected session");
    let w = windows.iter().enumerate().map(|(i, w)| {
        serde_json::json!({ "window_id": w.id, "label": w.label, "layout": w.layout,
            "focused_block_id": w.focus, "zoomed_block_id": w.zoomed, "active": i == active })
    });
    Value::Array(w.collect())
}

/// The same fields from the server's `session inspect`.
fn served(env: &Env, session: &str) -> Value {
    let v = env.view(session);
    let w = v["windows"].as_array().unwrap().iter().map(|w| {
        serde_json::json!({ "window_id": w["window_id"], "label": w["label"], "layout": w["layout"],
            "focused_block_id": w["focused_block_id"], "zoomed_block_id": w["zoomed_block_id"], "active": w["active"] })
    });
    Value::Array(w.collect())
}

/// The session view without what moves on its own (nothing does today; kept explicit).
fn tree(env: &Env, session: &str) -> Value {
    env.view(session)["windows"].clone()
}

#[gpui::test]
fn a_counter_outlives_the_app_and_its_layout_comes_back(cx: &mut TestAppContext) {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let pid = env.json(&["--json", "server", "status"])["pid"].clone();

    let (ws, vcx) = open(cx, &env);
    let sid = ws.read_with(vcx, |ws, _| ws.session_id()).expect("the app opened a session");
    vcx.simulate_keystrokes("cmd-d"); // a split, so the tree has a split node
    vcx.simulate_keystrokes("cmd-shift-enter"); // the new pane zoomed
    vcx.simulate_keystrokes("cmd-t"); // and a second tab
    vcx.simulate_keystrokes("cmd-1"); // back on the first tab
    let block = eventually(vcx, "the focused pane", |cx| ws.read_with(cx, |ws, _| ws.target_block()));
    // 300 lines first, so the first of them is in the scrollback, off any screen the app shows
    let counter = "seq -f pre-%g 1 300; i=0; while :; do i=$((i+1)); echo tick-$i; sleep 0.1; done\r";
    env.ok(&["send", "-b", &block, counter]);
    let seen = eventually(vcx, "ticks on the app's screen", |cx| {
        last_tick(&ws.read_with(cx, |ws, _| ws.screen_text(&block))?).filter(|n| *n >= 3)
    });
    let (before_app, before_server) = (shown(&ws, vcx), tree(&env, &sid));
    assert_eq!(before_app, served(&env, &sid), "the app shows the server's tabs, trees, focus and zoom");
    assert!(before_app.to_string().contains("\"zoomed_block_id\":\"block:"), "a zoom is part of it: {before_app}");

    quit(ws, vcx);

    // the program keeps running with no app attached
    let after_quit = last_tick(&env.capture(&block)).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    let later = last_tick(&env.capture(&block)).unwrap();
    assert!(later > after_quit && after_quit >= seen, "the counter kept counting: {seen} → {after_quit} → {later}");
    assert_eq!(env.json(&["--json", "server", "status"])["pid"], pid, "the same server");

    let (ws, vcx) = open(cx, &env);
    assert_eq!(ws.read_with(vcx, |ws, _| ws.session_id()), Some(sid.clone()), "the same session is selected");
    let shown_now = eventually(vcx, "a tick printed after the quit", |cx| {
        last_tick(&ws.read_with(cx, |ws, _| ws.screen_text(&block))?).filter(|n| *n > later)
    });
    assert!(shown_now > later);
    assert_eq!(tree(&env, &sid), before_server, "the server's tabs, splits and IDs are unchanged");
    assert_eq!(shown(&ws, vcx), before_app, "the app shows the same tabs, trees, focus and zoom");
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 1, "opening again made no new session");
    let all = env.ok(&["capture", "-b", &block]);
    assert!(all.lines().any(|l| l.trim() == "pre-1"), "scrollback from before the quit is kept");
    let on_screen = ws.read_with(vcx, |ws, _| ws.screen_text(&block)).unwrap();
    assert!(!on_screen.lines().any(|l| l.trim() == "pre-1"), "and pre-1 is in the scrollback, not on screen");
    quit(ws, vcx);
}

#[gpui::test]
fn a_cli_started_server_is_attached_to(cx: &mut TestAppContext) {
    let env = Env::new();
    let sid = env.ok(&["new", "busy"]); // autostarts a server holding a session
    let pid = env.json(&["--json", "server", "status"])["pid"].clone();
    let (ws, vcx) = open(cx, &env);
    assert_eq!(ws.read_with(vcx, |ws, _| ws.session_id()), Some(sid.clone()), "the app shows the CLI's session");
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 1, "and makes none of its own");
    assert_eq!(env.json(&["--json", "server", "status"])["pid"], pid, "nothing replaced the server");
    // the CLI keeps driving what the app shows
    env.ok(&["-s", &sid, "window", "new", "logs"]);
    eventually(vcx, "the CLI's tab in the app", |cx| {
        let w = ws.read_with(cx, |ws, _| ws.active_window())?;
        (w.label.as_deref() == Some("logs")).then_some(())
    });
    // closing the window detaches too (it quits the app, which leaves the server alone)
    vcx.update(|window, _| window.remove_window());
    drop(ws);
    vcx.cx.run_until_parked();
    assert_eq!(env.json(&["--json", "server", "status"])["pid"], pid, "the server outlives the window");
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 1, "and so does its session");
}

#[gpui::test]
fn quit_and_end_all_sessions_asks_once_and_ends_them(cx: &mut TestAppContext) {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let (_, block) = env.session(); // a session running `sleep 300`
    env.ok(&["new", "two"]);
    let pid = env.json(&["block", "inspect", &block, "--json"])["pid"].as_i64().unwrap() as i32;
    let (ws, vcx) = open(cx, &env);
    let names: Vec<String> = view::menus()
        .iter()
        .flat_map(|m| m.items.iter())
        .filter_map(|i| match i {
            gpui::MenuItem::Action { name, .. } => Some(name.to_string()),
            _ => None,
        })
        .collect();
    assert!(names.iter().any(|n| n == "Quit and End All Sessions…"), "the menu has the item: {names:?}");

    vcx.dispatch_action(QuitAndEndAll);
    let (msg, detail) = vcx.pending_prompt().expect("a prompt");
    assert!(format!("{msg} {detail}").contains("2 sessions"), "it says how many: {msg} / {detail}");
    vcx.simulate_prompt_answer("Cancel");
    vcx.run_until_parked();
    assert!(!vcx.has_pending_prompt(), "asked once");
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 2, "Cancel ends nothing");

    vcx.dispatch_action(QuitAndEndAll);
    vcx.simulate_prompt_answer("End All Sessions");
    vcx.run_until_parked();
    assert!(!vcx.has_pending_prompt(), "asked once");
    let end = Instant::now() + Duration::from_secs(10);
    while env.code(&["--autostart=false", "server", "status"]) == 0 {
        assert!(Instant::now() < end, "the server stopped");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(std::fs::symlink_metadata(env.socket()).is_err(), "the socket is gone");
    // SAFETY: signal 0 only asks whether the pid exists.
    let end = Instant::now() + Duration::from_secs(10);
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(Instant::now() < end, "the session's program ended");
        std::thread::sleep(Duration::from_millis(50));
    }
    drop(ws);
}

#[gpui::test]
fn the_server_saves_and_restores_and_the_app_shows_it(cx: &mut TestAppContext) {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let (ws, vcx) = open(cx, &env);
    let sid = ws.read_with(vcx, |ws, _| ws.session_id()).expect("a session");
    vcx.simulate_keystrokes("cmd-d");
    vcx.simulate_keystrokes("cmd-shift-d");
    let before = shown(&ws, vcx);
    assert_eq!(before, served(&env, &sid));
    quit(ws, vcx);

    env.ok(&["server", "stop"]); // saves
    env.ok(&["server", "start"]); // restores
    let (ws, vcx) = open(cx, &env);
    assert_eq!(ws.read_with(vcx, |ws, _| ws.session_id()), Some(sid.clone()), "the restored session");
    assert_eq!(shown(&ws, vcx), before, "the same tree, IDs and focus after a server restart");
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 1, "no extra session");
    quit(ws, vcx);
}
