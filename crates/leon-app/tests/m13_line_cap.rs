//! M13 criterion 8: a server (a remote one is a box you may not control) can't make the client
//! buffer a line past 16 MiB: replies and events are read by the same bounded reader.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use leon_app::remote::Remote;

/// A fake server that answers every request with a valid reply padded to `pad` bytes.
fn serve(listener: UnixListener, pad: usize) {
    std::thread::spawn(move || {
        for conn in listener.incoming() {
            let Ok(conn) = conn else { return };
            std::thread::spawn(move || {
                let mut r = BufReader::new(conn.try_clone().unwrap());
                let mut w = conn;
                let mut line = String::new();
                while r.read_line(&mut line).is_ok_and(|n| n > 0) {
                    let id = serde_json::from_str::<serde_json::Value>(&line).unwrap()["id"].clone();
                    let reply = serde_json::json!({ "id": id, "result": { "version": "1", "pad": "x".repeat(pad) } });
                    let mut bytes = serde_json::to_vec(&reply).unwrap();
                    bytes.push(b'\n');
                    if w.write_all(&bytes).is_err() {
                        return;
                    }
                    line.clear();
                }
            });
        }
    });
}

#[test]
fn a_reply_over_16_mib_is_refused() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    serve(UnixListener::bind(&socket).unwrap(), 17 << 20);
    let err = Remote::connect(&socket).err().expect("the client refuses the server");
    assert!(err.contains("too long"), "refused for its length: {err}");
}
