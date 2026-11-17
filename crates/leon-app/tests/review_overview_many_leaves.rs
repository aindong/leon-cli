//! Review proof (security): a server listing many tabs of many panes can't stall the overview's
//! render.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const WINDOWS: usize = 1024;
const LEAVES: usize = 32;

fn tree(w: usize, lo: usize, hi: usize, n: &mut usize) -> Value {
    if hi - lo == 1 {
        return json!({ "block": { "block_id": format!("block:{w}:{lo}") } });
    }
    *n += 1;
    let (id, mid) = (format!("split:{w}:{n}"), (lo + hi) / 2);
    let (before, after) = (tree(w, lo, mid, n), tree(w, mid, hi, n));
    json!({ "split": { "split_id": id, "direction": "horizontal", "ratio": 0.5, "before": before, "after": after } })
}

fn serve() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let windows: Vec<Value> = (0..WINDOWS)
        .map(|i| json!({ "window_id": format!("window:{i}"), "active": i == 0, "layout": tree(i, 0, LEAVES, &mut 0) }))
        .collect();
    let view = json!({ "session_id": "session:0", "windows": windows });
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let view = view.clone();
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                let mut r = BufReader::new(s);
                loop {
                    let mut line = String::new();
                    if r.read_line(&mut line).unwrap_or(0) == 0 {
                        return;
                    }
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                    let result = match v["method"].as_str().unwrap_or_default() {
                        "server.version" => json!({ "version": "fake" }),
                        "session.attach_all" => json!({ "seq": 0, "sessions": [] }),
                        "session.list" => json!({ "sessions": [{ "session_id": "session:0" }] }),
                        "session.view" => view.clone(),
                        "session.list_blocks" => json!({ "blocks": [] }),
                        "block.method" => json!({ "records": [] }),
                        _ => json!({}),
                    };
                    let mut out = serde_json::to_vec(&json!({ "id": v["id"], "result": result })).unwrap();
                    out.push(b'\n');
                    if w.write_all(&out).is_err() {
                        return;
                    }
                }
            });
        }
    });
    (dir, socket)
}

#[gpui::test]
fn many_tabs_of_many_panes_dont_stall_the_overview(cx: &mut gpui::TestAppContext) {
    let (dir, socket) = serve();
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
    std::env::set_var("LEON_RUNTIME_DIR", dir.path());
    std::env::set_var("LEON_STATE_DIR", dir.path());
    std::env::set_var("LEON_CONFIG", dir.path().join("no-init.lua"));
    let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(&socket).expect("the fake answers"));
    cx.update(leon_app::view::init);
    let (ws, cx) = cx.add_window_view(move |window, cx| leon_app::view::Workspace::new(remote, window, cx));
    // the server's settings come back on a socket thread, and applying them asks AppKit for the
    // window's blur the first time (a second or two in a test process): startup work, not the
    // overview's, so the clock starts once they are in
    let settled = Instant::now();
    while !ws.read_with(cx, |ws, _| ws.settings_applied()) && settled.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(10));
        cx.run_until_parked();
    }
    assert!(ws.read_with(cx, |ws, _| ws.settings_applied()), "the settings were applied");
    let t = Instant::now();
    cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action("client.overview.toggle", None, "test", window, cx)));
    cx.run_until_parked();
    let took = t.elapsed();
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_cards().len()), WINDOWS, "every tab is a card");
    assert!(took < Duration::from_secs(2), "{took:?} to open the overview on {WINDOWS} tabs of {LEAVES} panes");
}
