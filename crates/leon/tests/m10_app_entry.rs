//! M10 criterion 11 (.agent/plans/2026-11-14-m10-app-attaches.md): `leon app` starts or finds the
//! server before it opens a window, the way `leon server start` does, and honours
//! `--autostart=false`: with no server it exits 4 (no server, as every command does), names the
//! socket, and starts nothing.

mod support;

use support::Env;

#[test]
fn the_app_needs_a_server_and_autostart_off_starts_none() {
    let env = Env::new();
    for args in [&["--autostart=false", "app"][..], &["--autostart=false"][..]] {
        let out = env.run(args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(4), "leon {args:?}: {stderr}");
        assert!(stderr.contains(&env.socket().display().to_string()), "names the socket: {stderr}");
        assert!(!stderr.contains("leon server stop"), "no longer tells you to stop a server: {stderr}");
    }
    assert!(std::fs::symlink_metadata(env.socket()).is_err(), "no server was started");
}
