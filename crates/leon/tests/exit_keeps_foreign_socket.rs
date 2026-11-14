use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn leon(dir: &std::path::Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
    c.env("LEON_RUNTIME_DIR", dir)
        .env_remove("LEON_AUTOSTART")
        .env_remove("LEON_SESSION")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    c
}

fn wait_up(dir: &std::path::Path) {
    let end = Instant::now() + Duration::from_secs(10);
    while Instant::now() < end {
        if leon(dir).args(["--autostart=false", "server", "status"]).status().unwrap().success() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("server did not come up");
}

#[test]
fn exiting_server_leaves_a_newer_servers_socket_alone() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let mut old =
        leon(dir.path()).env("LEON_IDLE_EXIT_SECS", "1").args(["server", "run", "--autostarted"]).spawn().unwrap();
    wait_up(dir.path());
    // The old server loses its path (as in the bind/listen race) and a new one takes it.
    std::fs::remove_file(dir.path().join("server.sock")).unwrap();
    let mut new = leon(dir.path()).args(["server", "run"]).spawn().unwrap();
    wait_up(dir.path());
    let end = Instant::now() + Duration::from_secs(10);
    while old.try_wait().unwrap().is_none() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    let reachable = leon(dir.path()).args(["--autostart=false", "server", "status"]).status().unwrap().success();
    let _ = leon(dir.path()).args(["--autostart=false", "server", "stop"]).status();
    let _ = new.kill();
    let _ = new.wait();
    let _ = old.kill();
    let _ = old.wait();
    assert!(reachable, "the old server's exit unlinked the new server's socket");
}
