//! M16a criterion 4 (.agent/plans/2026-11-15-m16a-remote-ssh.md): the remote `leon` is found on
//! PATH, else in ~/.local/bin; with neither, a clear "install leon on HOST" and exit 4.

mod support;

use support::remote::Remote;

#[test]
fn missing_leon_says_install_it_then_home_local_bin_works() {
    let mut r = Remote::new();
    r.path = "/usr/bin:/bin:/usr/sbin:/sbin".into();
    let out = r.run(&["-H", "ssh://box", "ls"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(4), "{err}");
    assert!(err.contains("install leon on box"), "{err}");

    let bin = r.home().join(".local/bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_leon"), bin.join("leon")).unwrap();
    r.ok(&["-H", "ssh://box", "new", "found"]);
    assert_eq!(r.remote.json(&["--autostart=false", "ls", "--json"])[0]["label"], "found");
}
