//! A link's ssh gets its stdin, stdout and stderr and no other descriptor of this process. macOS
//! makes a pipe and marks it close-on-exec in two steps, so an ssh forked between the two (by
//! another connection's thread) kept that connection's pipes for as long as it ran: the other
//! `spawn` waited on its exec pipe, its connection was never served, and the client's first call
//! timed out (`impact_m16b_idle_link_stays` on the macOS runner).

use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

#[test]
fn ssh_inherits_no_descriptor_but_its_stdio() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    // a descriptor that is not close-on-exec, as a pipe half made on another thread is; a number
    // far past the ones `ls` opens for itself
    let mut pipe = [0; 2];
    // SAFETY: pipe fills the two descriptors; F_DUPFD copies one of ours.
    let planted = unsafe {
        assert_eq!(libc::pipe(pipe.as_mut_ptr()), 0);
        libc::fcntl(pipe[1], libc::F_DUPFD, 100)
    };
    assert!(planted >= 100, "{}", std::io::Error::last_os_error());

    // the fake ssh lists the descriptors it was started with
    let (ssh, seen) = (dir.path().join("ssh"), dir.path().join("fds"));
    let script = format!("#!/bin/sh\nls /dev/fd > '{0}.new' && mv '{0}.new' '{0}'\n", seen.display());
    std::fs::write(&ssh, script).unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::env::set_var("LEON_RUNTIME_DIR", dir.path());
    std::env::set_var("LEON_STATE_DIR", dir.path().join("state"));
    std::env::set_var("LEON_SSH", &ssh);

    let endpoint: leon_hosts::Endpoint = "ssh://box".parse().unwrap();
    let link = leon_hosts::Link::open("box", &endpoint, false).unwrap();
    let _conn = UnixStream::connect(link.socket()).unwrap();
    let end = Instant::now() + Duration::from_secs(30);
    while !seen.exists() {
        assert!(Instant::now() < end, "the fake ssh never ran");
        std::thread::sleep(Duration::from_millis(20));
    }
    let listed = std::fs::read_to_string(&seen).unwrap();
    let fds: Vec<i32> = listed.split_whitespace().filter_map(|fd| fd.parse().ok()).collect();
    assert!(fds.contains(&0) && fds.contains(&2), "the listing has ssh's stdio: {listed:?}");
    assert!(!fds.contains(&planted), "ssh inherited descriptor {planted}: {fds:?}");
}
