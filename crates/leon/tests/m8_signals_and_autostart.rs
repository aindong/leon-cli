//! M8 acceptance (.agent/plans/2026-11-14-m8-persist-speed.md, criteria 6 and 7): SIGTERM and
//! SIGINT save like `server stop`, and an autostarted server restores.

mod support;

use std::process::Stdio;
use std::time::{Duration, Instant};

use support::Env;

fn labels(env: &Env) -> Vec<String> {
    let ls = env.json(&["ls", "--json"]);
    ls.as_array().unwrap().iter().map(|s| s["label"].as_str().unwrap_or("").to_string()).collect()
}

#[test]
fn sigterm_and_sigint_save_the_sessions() {
    for (sig, name) in [(libc::SIGTERM, "term"), (libc::SIGINT, "int")] {
        let env = Env::new();
        let mut server = env.cmd().args(["server", "run"]).stdout(Stdio::null()).spawn().unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        while env.code(&["--autostart=false", "server", "status"]) != 0 {
            assert!(Instant::now() < end, "the foreground server didn't come up");
            std::thread::sleep(Duration::from_millis(50));
        }
        env.ok(&["new", name]);
        // SAFETY: only sends a signal to our own child.
        unsafe { libc::kill(server.id() as i32, sig) };
        let end = Instant::now() + Duration::from_secs(10);
        while server.try_wait().unwrap().is_none() {
            assert!(Instant::now() < end, "SIG{name} didn't stop the server");
            std::thread::sleep(Duration::from_millis(50));
        }
        env.ok(&["server", "start"]);
        assert_eq!(labels(&env), [name], "SIG{name}: the session was saved and restored");
    }
}

#[test]
fn an_autostarted_server_restores() {
    let env = Env::new();
    env.ok(&["new", "auto"]); // autostarts
    env.ok(&["server", "stop"]);
    assert_ne!(env.code(&["--autostart=false", "server", "status"]), 0, "stopped");
    assert_eq!(labels(&env), ["auto"], "ls autostarts a server, and it restores");
}
