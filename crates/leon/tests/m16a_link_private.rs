//! M16a criterion 11 (.agent/plans/2026-11-15-m16a-remote-ssh.md): the link is a Unix socket in
//! a private dir, never a TCP port, and its dir doesn't outlive the client (a killed client's dir
//! is removed by the next one).

mod support;

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use support::remote::Remote;

fn link_dirs(r: &Remote) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(r.local.dir.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("link-"))
        .collect()
}

fn mode(p: &std::path::Path) -> u32 {
    std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn the_link_is_private_and_not_tcp() {
    let r = Remote::new();
    r.ok(&["-H", "ssh://box", "ls"]);
    assert!(link_dirs(&r).is_empty(), "a finished command leaves no link dir: {:?}", link_dirs(&r));

    let ev = r.events(&["-H", "ssh://box"]);
    let dirs = link_dirs(&r);
    assert_eq!(dirs.len(), 1, "{dirs:?}");
    assert_eq!(mode(&dirs[0]), 0o700);
    let sockets: Vec<_> = std::fs::read_dir(&dirs[0]).unwrap().map(|e| e.unwrap().path()).collect();
    assert_eq!(sockets.len(), 1, "{sockets:?}");
    assert_eq!(mode(&sockets[0]), 0o600);
    let pid = ev.child.id().to_string();
    let out = Command::new("lsof").args(["-a", "-p", &pid, "-i"]).output().unwrap();
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "a TCP/UDP socket: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    // SAFETY: a signal to a process this test started
    unsafe { libc::kill(ev.child.id() as i32, libc::SIGINT) };
    drop(ev);
    assert_eq!(link_dirs(&r).len(), 1, "Ctrl-C skips the cleanup");
    r.ok(&["-H", "ssh://box", "ls"]);
    assert!(link_dirs(&r).is_empty(), "the next client removes a dead link's dir: {:?}", link_dirs(&r));
}
