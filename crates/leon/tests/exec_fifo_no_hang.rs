//! An executable FIFO named as a pane's program is refused without hanging the server: the error
//! path reads the program's first line to name a `#!` interpreter, and a blocking open of a FIFO
//! waits for a writer while the server's state lock is held.

mod support;

use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::sync::{mpsc, Arc};
use std::time::Duration;

#[test]
fn an_executable_fifo_is_refused_without_hanging_the_server() {
    let env = Arc::new(support::Env::new());
    let fifo = env.dir.path().join("fifo");
    assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
    std::fs::set_permissions(&fifo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (tx, rx) = mpsc::channel();
    let (e, f) = (env.clone(), fifo.clone());
    std::thread::spawn(move || tx.send(e.run(&["run", "--", f.to_str().unwrap()])).unwrap());
    let Ok(out) = rx.recv_timeout(Duration::from_secs(15)) else {
        // release the server's blocked open so it doesn't outlive the test
        let _ = std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NONBLOCK).open(&fifo);
        panic!("server hung opening the FIFO");
    };
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("bad_request") && err.contains("Permission denied"), "{err}");
}
