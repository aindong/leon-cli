//! Daily-driver F6 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 3): `leon wait --text`
//! waits until a block's screen shows the text, so a script can wait for a prompt before it types.

mod support;

use std::time::{Duration, Instant};
use support::Env;

#[test]
fn wait_text_returns_once_the_text_shows() {
    let env = Env::new();
    env.ok(&["new", "w"]);
    // the marker is built by the shell, so the command line itself never shows it
    let b =
        env.ok(&["-s", "w", "run", "--keep-open", "--", "sh", "-c", "sleep 1; echo ready-$((6*7)); exec sleep 300"]);
    let t = Instant::now();
    let o = env.cmd().args(["wait", "-b", &b, "--text", "ready-42", "--for", "10s"]).output().unwrap();
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(t.elapsed() >= Duration::from_millis(900), "returned before the text showed: {:?}", t.elapsed());
    assert!(env.capture(&b).contains("ready-42"));
}

#[test]
fn wait_text_gives_up_after_for() {
    let env = Env::new();
    env.ok(&["new", "w"]);
    let b = env.ok(&["-s", "w", "run", "--", "sleep", "300"]);
    let t = Instant::now();
    let o = env.cmd().args(["wait", "-b", &b, "--text", "absent", "--for", "300ms"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(t.elapsed() < Duration::from_secs(2), "{:?}", t.elapsed());
}

#[test]
fn wait_text_fails_when_the_block_closes_first() {
    let env = Env::new();
    env.ok(&["new", "w"]);
    let b = env.ok(&["-s", "w", "run", "--", "sleep", "1"]);
    let o = env.cmd().args(["wait", "-b", &b, "--text", "never-there", "--for", "10s"]).output().unwrap();
    assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(String::from_utf8_lossy(&o.stderr).contains("never-there"), "{}", String::from_utf8_lossy(&o.stderr));
}
