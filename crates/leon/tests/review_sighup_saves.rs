//! A foreground `leon server run` whose terminal closes gets SIGHUP: it saves like SIGTERM does.

mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use support::Env;

#[test]
fn sighup_saves_the_sessions() {
    let env = Env::new();
    let mut server = env.cmd().args(["server", "run"]).stdout(Stdio::null()).spawn().unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while env.code(&["--autostart=false", "server", "status"]) != 0 {
        assert!(Instant::now() < end, "the foreground server didn't come up");
        std::thread::sleep(Duration::from_millis(50));
    }
    env.ok(&["new", "hup"]);
    // SAFETY: only sends a signal to our own child.
    unsafe { libc::kill(server.id() as i32, libc::SIGHUP) };
    let end = Instant::now() + Duration::from_secs(10);
    while server.try_wait().unwrap().is_none() {
        assert!(Instant::now() < end, "SIGHUP didn't stop the server");
        std::thread::sleep(Duration::from_millis(50));
    }
    env.ok(&["server", "start"]);
    assert_eq!(env.json(&["ls", "--json"])[0]["label"], "hup", "saved on SIGHUP and restored");
}
