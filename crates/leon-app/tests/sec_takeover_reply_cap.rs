//! Review proof: the takeover bounds the reply it reads from whatever owns the socket.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::time::Duration;

#[test]
fn takeover_bounds_the_reply_it_reads() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    // A fake "server" that answers every request with an endless line and no newline.
    std::thread::spawn(move || {
        for s in listener.incoming() {
            let Ok(mut s) = s else { return };
            std::thread::spawn(move || {
                let chunk = vec![b'x'; 64 << 10];
                while s.write_all(&chunk).is_ok() {
                    std::thread::sleep(Duration::from_millis(10));
                }
            });
        }
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let path = socket.clone();
    std::thread::spawn(move || {
        let _ = tx.send(leon_app::embedded::start_at(path).is_ok());
    });
    let returned = rx.recv_timeout(Duration::from_secs(15));
    assert!(returned.is_ok(), "start_at kept reading an endless reply: unbounded read in embedded::request");
    std::mem::forget(dir);
}
