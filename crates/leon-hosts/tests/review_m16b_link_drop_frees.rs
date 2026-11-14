//! M16b review: a dropped Link must close its socket and end its accept thread. The app opens a
//! new Link on every retry to a host that is down, for as long as it runs.

use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

fn open_fds() -> usize {
    std::fs::read_dir("/dev/fd").unwrap().count()
}

#[test]
fn a_dropped_link_leaves_no_fd_behind() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::env::set_var("LEON_RUNTIME_DIR", dir.path());
    std::env::set_var("LEON_SSH", "/usr/bin/false"); // nothing may run ssh for real here
    let endpoint: leon_hosts::Endpoint = "ssh://unreachable-box".parse().unwrap();
    drop(leon_hosts::Link::open("warmup", &endpoint, true).unwrap());
    let before = open_fds();
    for _ in 0..50 {
        drop(leon_hosts::Link::open("unreachable-box", &endpoint, true).unwrap());
    }
    let end = Instant::now() + Duration::from_secs(5);
    while open_fds() > before + 2 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    let after = open_fds();
    assert!(after <= before + 2, "50 dropped links leaked {} fds", after.saturating_sub(before));
}
