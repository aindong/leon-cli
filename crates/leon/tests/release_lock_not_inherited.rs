//! Impact proof for holding `server.lock` for the server's life: no program it runs inherits the
//! lock. If one did, a program outliving a killed server would hold the lock for ever, and every
//! later server would refuse to start ("held by a leon server that isn't answering").

mod support;

use std::time::{Duration, Instant};

use support::Env;

#[test]
fn a_program_outliving_a_killed_server_doesnt_keep_its_lock() {
    let env = Env::new();
    let pidfile = env.dir.path().join("orphan.pid");
    let script = format!("trap '' HUP; echo $$ > '{}'; exec sleep 60", pidfile.display());
    env.ok(&["run", "--", "sh", "-c", &script]);
    let end = Instant::now() + Duration::from_secs(10);
    while !pidfile.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    let orphan: i32 = std::fs::read_to_string(&pidfile).unwrap().trim().parse().unwrap();
    let server = env.json(&["server", "status", "--json"])["pid"].as_i64().unwrap() as i32;
    // SAFETY: plain kill(2) on pids this test started.
    unsafe { libc::kill(server, libc::SIGKILL) };
    while env.code(&["--autostart=false", "server", "status"]) == 0 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    // SAFETY: as above; the orphan survived its hang-up.
    let alive = unsafe { libc::kill(orphan, 0) } == 0;

    let start = env.run(&["server", "start", "--restore=false"]);
    unsafe { libc::kill(orphan, libc::SIGKILL) };
    assert!(alive, "the program didn't outlive the server, so this proves nothing");
    assert!(start.status.success(), "a new server didn't start: {}", String::from_utf8_lossy(&start.stderr));
    env.ok(&["server", "status"]);
}
