//! Review (security): `leon server start` (and so `leon app`, which runs it first) talks only to a
//! server whose socket sits in a private dir; one in a dir others can write to could be anyone's.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

#[test]
fn leon_refuses_a_server_socket_in_a_dir_others_can_write() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let listener = UnixListener::bind(dir.path().join("server.sock")).unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut w = stream.try_clone().unwrap();
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    let id =
                        serde_json::from_str::<serde_json::Value>(&line).map(|v| v["id"].clone()).unwrap_or_default();
                    let _ = writeln!(w, "{}", serde_json::json!({ "id": id, "result": {} }));
                }
            });
        }
    });
    let state = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["server", "start"])
        .env("LEON_RUNTIME_DIR", dir.path())
        .env("LEON_STATE_DIR", state.path())
        .env("LEON_CONFIG", state.path().join("no-init.lua"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success() && stderr.contains("unsafe runtime directory"),
        "leon talked to a server in a world-writable dir: status {:?}, stderr {stderr}",
        out.status
    );
}
