//! Review (cross-model): the app types into whatever answers its socket, so it attaches only to a
//! socket in a private dir, as the server binds only in one. A socket in a dir others can write to
//! could belong to anyone.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;

use leon_app::remote::Remote;

#[test]
fn the_app_refuses_a_socket_in_an_open_dir() {
    let parent = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let open = parent.path().join("open");
    std::fs::create_dir(&open).unwrap();
    std::fs::set_permissions(&open, std::fs::Permissions::from_mode(0o777)).unwrap();
    let socket = open.join("server.sock");
    // a stand-in that answers every request as an empty leon server would
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut w = s.try_clone().unwrap();
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                    let reply = serde_json::json!({ "id": v["id"], "result": { "sessions": [] } });
                    let _ = writeln!(w, "{reply}");
                }
            });
        }
    });
    let attached = Remote::connect(&socket);
    assert!(attached.is_err(), "the app attached to a socket in a dir anyone can write to");
}
