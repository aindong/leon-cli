//! Review proof: a `client.keymap` ask that fails is made again (the 1 s keymap clock did).
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{px, size, TestAppContext};
use leon_app::view::{self, Workspace};
use serde_json::{json, Value};

fn serve(socket: &std::path::Path, asks: Arc<AtomicUsize>) {
    let listener = UnixListener::bind(socket).unwrap();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let asks = asks.clone();
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                for line in BufReader::new(s).lines() {
                    let Ok(line) = line else { return };
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                    let (p, method) = (&v["params"], v["method"].as_str().unwrap_or_default());
                    let window = json!({ "window_id": "window:0", "active": true, "focused_block_id": "block:0",
                                         "layout": { "block": { "block_id": "block:0" } } });
                    let block = json!({ "block_id": "block:0", "window_id": "window:0", "command": ["sh"] });
                    let mut out = match method {
                        "server.version" => json!({ "result": { "version": "fake" } }),
                        "session.attach_all" => json!({ "result": { "seq": 0, "sessions": [] } }),
                        "session.list" => json!({ "result": { "sessions": [{ "session_id": "session:0" }] } }),
                        "session.view" => json!({ "result": { "session_id": "session:0", "windows": [window] } }),
                        "session.list_blocks" => json!({ "result": { "blocks": [block] } }),
                        "block.method" if p["method"] == "dev.leon.terminal.program_status" => {
                            json!({ "result": { "records": [] } })
                        }
                        "block.watch" => continue,
                        "client.keymap" if asks.fetch_add(1, Ordering::SeqCst) == 0 => {
                            json!({ "error": { "code": 503, "error": "busy", "message": "the config is loading" } })
                        }
                        _ => json!({ "result": {} }),
                    };
                    out["id"] = v["id"].clone();
                    let mut l = serde_json::to_vec(&out).unwrap();
                    l.push(b'\n');
                    if w.write_all(&l).is_err() {
                        return;
                    }
                }
            });
        }
    });
}

#[gpui::test]
fn a_failed_keymap_ask_is_made_again(cx: &mut TestAppContext) {
    let dir =
        tempfile::Builder::new().permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700)).tempdir().unwrap();
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
    std::env::set_var("LEON_STATE_DIR", dir.path());
    std::env::set_var("LEON_RUNTIME_DIR", dir.path());
    let socket = dir.path().join("server.sock");
    let asks = Arc::new(AtomicUsize::new(0));
    serve(&socket, asks.clone());
    cx.update(view::init);
    let (_ws, cx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(&socket).expect("the fake answers the handshake"));
        Workspace::with_settings(remote, None, window, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    let end = Instant::now() + Duration::from_secs(5);
    while asks.load(Ordering::SeqCst) < 2 && Instant::now() < end {
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    let n = asks.load(Ordering::SeqCst);
    assert!(n >= 2, "client.keymap was asked {n} time(s): the failed first ask was never made again");
}
