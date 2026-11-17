//! Review proof (security): the overview never asks one server to watch more blocks than
//! leon-server accepts (256).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use serde_json::{json, Value};

const WINDOWS: usize = 1024;

fn serve() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
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
                        "session.view" => {
                            let windows: Vec<Value> = (0..WINDOWS)
                                .map(|i| {
                                    json!({ "window_id": format!("window:{i}"), "active": i == 0,
                                    "layout": { "block": { "block_id": format!("block:{i}") } } })
                                })
                                .collect();
                            json!({ "session_id": "session:0", "windows": windows })
                        }
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
fn the_overview_watches_at_most_what_a_server_accepts(cx: &mut gpui::TestAppContext) {
    let (dir, socket) = serve();
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
    std::env::set_var("LEON_RUNTIME_DIR", dir.path());
    std::env::set_var("LEON_STATE_DIR", dir.path());
    std::env::set_var("LEON_CONFIG", dir.path().join("no-init.lua"));
    let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(&socket).expect("the fake answers"));
    cx.update(leon_app::view::init);
    let (ws, cx) = cx.add_window_view(move |window, cx| leon_app::view::Workspace::new(remote, window, cx));
    cx.run_until_parked();
    cx.update(|window, cx| ws.update(cx, |ws, cx| ws.run_action("client.overview.toggle", None, "test", window, cx)));
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.overview_cards().len()), WINDOWS, "every tab is a card");
    let watched = ws.read_with(cx, |ws, _| ws.watching().len());
    assert!(watched <= 256, "asked one server to watch {watched} blocks; leon-server refuses more than 256");
}
