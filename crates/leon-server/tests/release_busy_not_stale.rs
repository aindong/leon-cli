//! Release audit: a live server that is busy (not accepting, its backlog full) is not taken for a
//! stale socket by a second server, which would unlink it and leave every program unreachable.

// macOS only: a full unix backlog refuses connects there; on Linux a blocking connect waits instead.
#![cfg(target_os = "macos")]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;

#[tokio::test]
async fn a_busy_live_server_is_not_taken_for_stale() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let sock = dir.path().join("server.sock");
    // bound and never accepting: a server still restoring its sessions, or stuck on its state lock
    let live = leon_server::serve::listen(&sock).await.unwrap();
    let ino = std::fs::metadata(&sock).unwrap().ino();
    let mut held = Vec::new();
    while let Ok(s) = UnixStream::connect(&sock) {
        held.push(s);
        assert!(held.len() < 100_000, "the backlog never filled");
    }
    let second = leon_server::serve::listen(&sock).await;
    assert!(second.is_err(), "a second server took a busy live server's socket for stale");
    assert_eq!(std::fs::metadata(&sock).unwrap().ino(), ino, "the live server's socket is gone");
    drop(live);
}

use std::os::unix::fs::MetadataExt;
