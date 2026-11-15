//! Daily-driver criterion 3 (.agent/plans/2026-11-15-daily-driver-fixes.md): with `-H`, this
//! machine's working directory means nothing on the other machine. `run` and `split` start in the
//! host's home unless `--cwd` names a directory there; `new --cwd` isn't checked on this disk.

mod support;

use support::remote::Remote;

const H: &str = "ssh://box";

/// The directory a remote `sh` started in, from its screen.
fn pwd_of(r: &Remote, block: &str) -> String {
    let line = || {
        let text = r.ok(&["-H", H, "capture", "-b", block]);
        text.lines().find(|l| l.starts_with("D=")).unwrap_or_default().to_string()
    };
    r.until("the remote pwd", || !line().is_empty());
    line().trim_start_matches("D=").to_string()
}

#[test]
fn run_and_split_start_in_the_hosts_home_not_this_machines_cwd() {
    let r = Remote::new();
    r.ok(&["-H", H, "new", "r1"]);
    // a directory that exists only here: the caller's own temp dir
    let here = r.local.dir.path().join("only-here");
    std::fs::create_dir(&here).unwrap();
    let home = r.home().canonicalize().unwrap();
    let in_here = |args: &[&str]| {
        let out = r.cmd().current_dir(&here).args(args).output().unwrap();
        assert!(out.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let b = in_here(&["-H", H, "-s", "r1", "run", "--keep-open", "--", "sh", "-c", "echo D=$(pwd -P)"]);
    assert_eq!(pwd_of(&r, &b), home.to_str().unwrap(), "run");
    let b = in_here(&["-H", H, "-s", "r1", "split", "--keep-open", "--", "sh", "-c", "echo D=$(pwd -P)"]);
    assert_eq!(pwd_of(&r, &b), home.to_str().unwrap(), "split");
}

#[test]
fn a_cwd_given_with_dash_h_is_the_hosts_and_must_be_absolute() {
    let r = Remote::new();
    // relative to what? this machine's cwd isn't there: refused before any ssh
    let out = r.run(&["-H", H, "new", "r1", "--cwd", "src"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(String::from_utf8_lossy(&out.stderr).contains("absolute"), "{}", String::from_utf8_lossy(&out.stderr));
    let out = r.run(&["-H", H, "run", "--cwd", "src", "--", "true"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));

    // an absolute path is the host's to check, not this disk's
    let remote_only = "/dd-leon-no-such-dir-here";
    assert!(!std::path::Path::new(remote_only).exists());
    let calls = r.ssh_calls().len();
    let out = r.run(&["-H", H, "new", "r2", "--cwd", remote_only]);
    assert!(r.ssh_calls().len() > calls, "the host was asked: {}", String::from_utf8_lossy(&out.stderr));
    assert_ne!(out.status.code(), Some(2), "not refused here as a bad local path");

    let dir = r.home().join("proj");
    std::fs::create_dir(&dir).unwrap();
    let dir = dir.canonicalize().unwrap();
    r.ok(&["-H", H, "new", "r3", "--cwd", dir.to_str().unwrap()]);
    let b = r.ok(&[
        "-H",
        H,
        "-s",
        "r3",
        "run",
        "--keep-open",
        "--cwd",
        dir.to_str().unwrap(),
        "--",
        "sh",
        "-c",
        "echo D=$(pwd -P)",
    ]);
    assert_eq!(pwd_of(&r, &b), dir.to_str().unwrap());
}

#[test]
fn a_relative_cwd_with_dash_h_is_refused_before_any_connection() {
    let r = Remote::new();
    for cmd in [&["run", "--cwd", "src", "--", "true"][..], &["split", "--cwd", "src", "--", "true"][..]] {
        let args: Vec<&str> = ["-H", "ssh://unreachable-box"].iter().chain(cmd).copied().collect();
        let out = r.run(&args);
        assert_eq!(out.status.code(), Some(2), "{cmd:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert!(r.ssh_calls().is_empty(), "no ssh for a usage error: {:?}", r.ssh_calls());
}
