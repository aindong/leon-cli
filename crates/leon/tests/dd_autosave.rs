//! Daily-driver criterion 4 (.agent/plans/2026-11-15-daily-driver-fixes.md): sessions are saved
//! while the server runs, not only when it stops, so a crash (SIGKILL) loses at most a few seconds
//! and never brings back the save from the stop before.

mod support;

use std::time::{Duration, Instant};

use support::Env;

#[test]
fn a_crashed_server_comes_back_with_its_latest_sessions() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    env.ok(&["new", "kept"]);
    env.ok(&["new", "gone"]);
    env.ok(&["kill", "gone"]);
    env.ok(&["new", "kept-later"]);

    let file = env.state_dir().join("sessions.json");
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        if text.contains("\"kept-later\"") && !text.contains("\"gone\"") {
            break;
        }
        assert!(Instant::now() < end, "not saved within 5 s while running:\n{text}");
        std::thread::sleep(Duration::from_millis(100));
    }

    let pid = env.json(&["server", "status", "--json"])["pid"].as_i64().unwrap() as i32;
    // SAFETY: a signal to the server this test started
    unsafe { libc::kill(pid, libc::SIGKILL) };
    let end = Instant::now() + Duration::from_secs(10);
    while env.code(&["--autostart=false", "server", "status"]) == 0 {
        assert!(Instant::now() < end, "the server didn't die");
        std::thread::sleep(Duration::from_millis(50));
    }

    env.ok(&["server", "start"]);
    let labels: Vec<String> = env
        .json(&["ls", "--json"])
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["label"].as_str().unwrap_or_default().to_string())
        .collect();
    assert!(labels.contains(&"kept".to_string()) && labels.contains(&"kept-later".to_string()), "{labels:?}");
    assert!(!labels.contains(&"gone".to_string()), "a killed session stays gone: {labels:?}");
}

#[test]
fn an_idle_server_does_not_rewrite_its_file() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    env.ok(&["new", "quiet"]);
    let file = env.state_dir().join("sessions.json");
    let end = Instant::now() + Duration::from_secs(5);
    while !file.exists() {
        assert!(Instant::now() < end, "never saved");
        std::thread::sleep(Duration::from_millis(100));
    }
    let first = std::fs::metadata(&file).unwrap().modified().unwrap();
    // nothing changes for a few seconds: no write (the text check runs once a minute at most)
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(std::fs::metadata(&file).unwrap().modified().unwrap(), first, "rewritten while nothing changed");
}

#[test]
fn stop_discard_leaves_no_file_even_while_a_program_holds_the_shutdown() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    // ignores SIGHUP, so ending the programs waits on it after the loop has stopped
    env.ok(&["run", "--", "sh", "-c", "trap '' HUP; sleep 3"]);
    env.ok(&["new", "second"]);
    let file = env.state_dir().join("sessions.json");
    let end = Instant::now() + Duration::from_secs(5);
    while !std::fs::read_to_string(&file).unwrap_or_default().contains("\"second\"") {
        assert!(Instant::now() < end, "never saved");
        std::thread::sleep(Duration::from_millis(100));
    }
    env.ok(&["server", "stop", "--discard"]);
    let end = Instant::now() + Duration::from_secs(10);
    while env.code(&["--autostart=false", "server", "status"]) == 0 || env.socket().exists() {
        assert!(Instant::now() < end, "the server didn't stop");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!file.exists(), "discard forgets the sessions; nothing wrote them back");
}
