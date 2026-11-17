//! What the M9 view tests share: a private embedded server, the workspace window, and waits.
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{px, size, Entity, TestAppContext, VisualTestContext};
use leon_app::embedded::{self, Embedded};
use leon_app::view::{self, Workspace};
use leon_core::terminal::Terminal;
use leon_core::Window;

pub struct Setup {
    pub dir: tempfile::TempDir,
    pub app: Rc<Embedded>,
}

fn private_dir() -> tempfile::TempDir {
    tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap()
}

/// A server with no config.
pub fn setup() -> Setup {
    let dir = private_dir();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    Setup { dir, app }
}

/// A server that loads `init_lua` as its config; returns once the load has finished.
pub fn setup_with_config(init_lua: &str) -> Setup {
    let dir = private_dir();
    let config = dir.path().join("init.lua");
    std::fs::write(&config, init_lua).unwrap();
    let app = Rc::new(embedded::start_configured(dir.path().join("server.sock"), config).unwrap());
    // client.keymap waits for the load that started with the server
    let km = app.call("client.keymap", serde_json::json!({})).unwrap();
    assert!(km["generation"].as_u64().unwrap() >= 1, "the config loaded: {km}");
    Setup { dir, app }
}

/// Never this machine's Tailscale: Add Host… reads the tailnet when a client is found.
pub fn no_tailscale() {
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
}

pub fn open<'a>(cx: &'a mut TestAppContext, app: &Rc<Embedded>) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    no_tailscale();
    cx.update(view::init);
    let app = app.clone();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        Workspace::new(Rc::new(leon_app::remote::Remote::connect(app.socket()).expect("the app attaches")), window, cx)
    });
    vcx.simulate_resize(size(px(1400.), px(900.)));
    vcx.run_until_parked();
    (ws, vcx)
}

/// The selected session's active window, and how many tabs the session has.
pub fn window(app: &Embedded, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (Window, usize) {
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).expect("a session is selected");
    let st = app.ctx().state();
    let s = st.session(&sid).unwrap();
    (s.windows[s.active].clone(), s.windows.len())
}

pub fn leaves(w: &Window) -> Vec<String> {
    w.layout.as_ref().map(|t| t.leaves().into_iter().map(String::from).collect()).unwrap_or_default()
}

pub fn term(app: &Embedded, block_id: &str) -> Arc<Terminal> {
    app.ctx().state().block(block_id).unwrap().1.terminal.clone()
}

pub fn focused(app: &Embedded, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Arc<Terminal> {
    let id = ws.read_with(cx, |ws, _| ws.target_block()).expect("the active window has a terminal");
    term(app, &id)
}

/// Draw the animation frame the app asked for, `step` later on GPUI's clock. GPUI's test window
/// never runs `on_next_frame`, so this does what `request_animation_frame` would (notify the
/// view that asked); it fails if the last render asked for no frame, so a motion only moves here
/// when the app itself asks (ADR R6, R10).
pub fn frame(ws: &Entity<Workspace>, cx: &mut VisualTestContext, step: Duration) {
    assert!(ws.read_with(cx, |ws, _| ws.frame_asked()), "the app asked for no frame");
    cx.executor().advance_clock(step);
    ws.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
}

/// Draw the frames the app asks for, each a display frame (16 ms) on, until it asks for none: a
/// motion runs to its end. Returns how many; fails past 100 (a motion that never ends).
pub fn frames(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> usize {
    let mut n = 0;
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        assert!(n < 100, "the app kept asking for frames");
        frame(ws, cx, Duration::from_millis(16));
        n += 1;
    }
    n
}

/// Move GPUI's clock on by `step`, drawing the animation frame the app asked for if it asked for
/// one (`frame`); with nothing moving it only runs what the clock set off (timers).
pub fn tick(ws: &Entity<Workspace>, cx: &mut VisualTestContext, step: Duration) {
    if ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(ws, cx, step);
    } else {
        cx.executor().advance_clock(step);
        cx.run_until_parked();
    }
}

/// Poll `pred` for up to 10 s, letting the window run between tries; returns whether it came
/// true. The app hears of changes made outside it (a program's status) by events, a moment later.
pub fn shows(cx: &mut VisualTestContext, mut pred: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    while !pred(cx) && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    pred(cx)
}

/// Poll `pred` for up to 30 s; returns whether it came true (the cap is for a machine busy with
/// other builds: a fast run returns as soon as `pred` holds).
pub fn eventually(mut pred: impl FnMut() -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(30);
    while Instant::now() < end {
        if pred() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    pred()
}

pub fn has_line(t: &Terminal, line: &str) -> bool {
    t.text(true).lines().any(|l| l.trim() == line)
}

/// Run `line` in `t` and wait for `marker` on a line of its own.
pub fn run_until(t: &Terminal, line: &str, marker: &str) {
    t.write(format!("{line}\r").as_bytes()).unwrap();
    assert!(eventually(|| has_line(t, marker)), "no {marker:?} after {line:?}:\n{}", t.text(true));
}

/// The real path of `p` (macOS temp dirs sit behind the /var -> /private/var link).
pub fn real(p: &Path) -> String {
    std::fs::canonicalize(p).unwrap().display().to_string()
}

/// Wait until `t` has drawn nothing new for 150 ms (a prompt finished drawing).
pub fn settle(t: &Terminal) {
    let end = Instant::now() + Duration::from_secs(10);
    let (mut last, mut since) = (t.generation(), Instant::now());
    while Instant::now() < end && since.elapsed() < Duration::from_millis(150) {
        std::thread::sleep(Duration::from_millis(10));
        if t.generation() != last {
            (last, since) = (t.generation(), Instant::now());
        }
    }
}

/// A second session the app has heard of but doesn't show: (its id, its first tab's block).
pub fn hidden_session(app: &Embedded, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (String, String) {
    let sid = app.call("session.create", serde_json::json!({ "label": "hidden" })).unwrap()["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    let tab = app.call("session.new_window", serde_json::json!({ "session_id": sid })).unwrap();
    let block = tab["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_title(&block).is_some())), "the app heard of the session");
    (sid, block)
}

/// Whether `t` keeps `size` for 500 ms while the window runs (a resize lands in a few ms).
pub fn stays(cx: &mut VisualTestContext, t: &Terminal, size: (u16, u16)) -> bool {
    let end = Instant::now() + Duration::from_millis(500);
    while Instant::now() < end {
        cx.run_until_parked();
        if t.size() != size {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    t.size() == size
}

/// Panes run a plain `sh` with a private home that holds no shell config, never this machine's:
/// a login zsh's prompt may set the title to user@host or swallow a line typed before it. Call it
/// before `setup()`. It sets the process's environment once, before the first shell starts.
pub fn plain_shell() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let home = Path::new(env!("CARGO_TARGET_TMPDIR")).join("plain-home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var("SHELL", "/bin/sh");
        std::env::set_var("HOME", &home);
        // a login sh reads $ENV, and /etc/bashrc reads /etc/bashrc_$TERM_PROGRAM
        std::env::remove_var("ENV");
        std::env::remove_var("TERM_PROGRAM");
    });
}

/// What `leon config set KEY VALUE` does: rewrite init.lua, then reload the running server.
pub fn config_set(s: &Setup, key: &str, value: serde_json::Value) {
    leon_lua::rewrite::write_setting(&s.dir.path().join("init.lua"), key, &value).unwrap();
    s.app.call("server.config_reload", serde_json::json!({})).unwrap();
}
