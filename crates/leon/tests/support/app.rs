//! M16b: the app's workspace on an in-process local server, with hosts reached through the fake
//! ssh of `support::remote`. The link and the hosts list read process-wide env (`LEON_SSH`,
//! `LEON_RUNTIME_DIR`, `LEON_STATE_DIR`), so each test file holds one test. Include it with
//! `#[path = "support/app.rs"] mod app;` next to `mod support;`.
#![allow(dead_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{px, size, Entity, TestAppContext, VisualTestContext};
use leon_app::embedded::{self, Embedded};
use leon_app::hosts::HostState;
use leon_app::view::{self, Workspace};
use leon_hosts::Host;

use crate::support::remote::Remote;

pub struct Hosted {
    pub r: Remote,
    /// This Mac's server, in process.
    pub local: Rc<Embedded>,
    pub dir: tempfile::TempDir,
}

/// The fake ssh's variables and this machine's dirs, set for this process (the app's links read
/// them), and a local in-process server.
pub fn hosted() -> Hosted {
    let r = Remote::new();
    for (k, v) in r.vars() {
        std::env::set_var(k, v);
    }
    std::env::set_var("LEON_RUNTIME_DIR", r.local.dir.path());
    std::env::set_var("LEON_STATE_DIR", r.local.state_dir());
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let local = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    Hosted { r, local, dir }
}

pub fn host(label: &str, endpoint: &str) -> Host {
    Host { label: label.into(), endpoints: vec![endpoint.into()], icon: None }
}

pub fn open<'a>(
    cx: &'a mut TestAppContext,
    h: &Hosted,
    hosts: Vec<Host>,
    settings: Option<PathBuf>,
) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    cx.update(view::init);
    let socket = h.local.socket().to_path_buf();
    let (ws, vcx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(&socket).expect("the app attaches"));
        Workspace::with_hosts(remote, settings, hosts, window, cx)
    });
    vcx.simulate_resize(size(px(1400.), px(900.)));
    vcx.run_until_parked();
    (ws, vcx)
}

/// Poll `pred` for up to 15 s, letting the window run between tries; returns whether it came true.
pub fn shows(cx: &mut VisualTestContext, mut pred: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(15);
    while !pred(cx) && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    pred(cx)
}

/// A host's state as the sidebar shows it.
pub fn state(ws: &Entity<Workspace>, cx: &mut VisualTestContext, label: &str) -> Option<HostState> {
    ws.read_with(cx, |ws, _| ws.host_rows().into_iter().find(|h| h.label == label).map(|h| h.state))
}

pub fn wait_connected(ws: &Entity<Workspace>, cx: &mut VisualTestContext, label: &str) {
    let ok = shows(cx, |cx| state(ws, cx, label) == Some(HostState::Connected));
    assert!(ok, "{label} never connected: {:?}", state(ws, cx, label));
}

/// Whether the last frame drew the element with debug selector `sel`.
pub fn drawn(cx: &mut VisualTestContext, sel: &str) -> bool {
    cx.run_until_parked();
    view::drawn().contains(sel)
}

/// Run an app action by name, as a key or the palette would.
pub fn act(ws: &Entity<Workspace>, cx: &mut VisualTestContext, name: &str, args: Option<serde_json::Value>) {
    cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action(name, args.as_ref(), "test", window, cx)));
    cx.run_until_parked();
}

/// The session ids a server lists (`leon ls --json`), straight from its own dirs.
pub fn session_ids(env: &crate::support::Env) -> Vec<String> {
    let v = env.json(&["ls", "--json"]);
    v.as_array().unwrap().iter().map(|s| s["session_id"].as_str().unwrap().to_string()).collect()
}

/// A session labelled `label` on `env`'s server whose one block runs `sh -c script`; returns
/// (session id, block id).
pub fn session_running(env: &crate::support::Env, label: &str, script: &str) -> (String, String) {
    let p = serde_json::json!({ "label": label, "layout": { "block": { "options": { "command": ["sh", "-c", script] } } } });
    let made = env.json(&["api", "call", "session.create", &p.to_string()]);
    (made["session_id"].as_str().unwrap().into(), made["block_id"].as_str().unwrap().into())
}

/// Select a session (its active tab) in the workspace.
pub fn select(ws: &Entity<Workspace>, cx: &mut VisualTestContext, sid: &str) {
    ws.update(cx, |ws, cx| ws.select(sid, None, cx));
    cx.run_until_parked();
}

/// Send `sig` to every transport still running (each fake ssh became `leon server proxy`).
pub fn signal_transports(r: &Remote, sig: i32) -> usize {
    let pids = std::fs::read_to_string(r.fake.path().join("pids")).unwrap_or_default();
    let mut n = 0;
    for pid in pids.lines().filter_map(|p| p.trim().parse::<i32>().ok()) {
        let out = std::process::Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().unwrap();
        if String::from_utf8_lossy(&out.stdout).contains("server proxy") {
            // SAFETY: a signal to a process this test started
            unsafe { libc::kill(pid, sig) };
            n += 1;
        }
    }
    n
}
