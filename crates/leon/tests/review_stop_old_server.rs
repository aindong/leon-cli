//! A new `leon server stop` against an older server whose `server.stop` takes no params.
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use serde_json::json;

#[test]
fn plain_stop_sends_no_params_an_old_server_refuses() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let sock = dir.path().join("server.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let path = sock.clone();
    std::thread::spawn(move || {
        for s in listener.incoming() {
            let Ok(s) = s else { return };
            let mut r = BufReader::new(s);
            let mut line = String::new();
            loop {
                line.clear();
                if r.read_line(&mut line).unwrap_or(0) == 0 {
                    break;
                }
                let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                let empty = v["params"].as_object().is_none_or(|o| o.is_empty());
                let reply = match (v["method"].as_str().unwrap_or(""), empty) {
                    ("server.stop", true) => {
                        let _ = std::fs::remove_file(&path);
                        json!({ "id": v["id"], "result": {} })
                    }
                    (_, true) => json!({ "id": v["id"], "result": { "version": "0.old" } }),
                    (m, false) => json!({ "id": v["id"], "error": {
                        "status": 400, "code": "bad_request",
                        "message": format!("{m}: unknown field `discard`, there are no fields") } }),
                };
                let mut out = serde_json::to_vec(&reply).unwrap();
                out.push(b'\n');
                r.get_mut().write_all(&out).unwrap();
            }
        }
    });
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["server", "stop"])
        .env("LEON_RUNTIME_DIR", dir.path())
        .env("LEON_STATE_DIR", dir.path().join("state"))
        .env("LEON_AUTOSTART", "0")
        .env_remove("LEON_SESSION")
        .env_remove("LEON_BLOCK")
        .output()
        .unwrap();
    assert!(out.status.success(), "OLD_SERVER_STOP_REFUSED: {}", String::from_utf8_lossy(&out.stderr));
}
