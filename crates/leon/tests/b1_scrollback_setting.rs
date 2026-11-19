//! Scrollback B1, criterion 7: the `scrollback` setting caps a pane's history. `leon config`
//! reads and writes it, a value out of range is refused, `server.settings` reports it, and a
//! running server applies it at once: a lowered value trims what is kept.

mod support;

use std::path::Path;
use std::process::Output;
use std::time::{Duration, Instant};

use serde_json::Value;
use support::Env;

fn config(env: &Env, path: &Path, args: &[&str]) -> Output {
    env.cmd().env("LEON_CONFIG", path).arg("config").args(args).output().unwrap()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

fn history(env: &Env, block: &str) -> Value {
    env.json(&["-b", block, "block", "call", "lines", r#"{"from": 0, "count": 0}"#])
}

/// Run `seq 1 N` in a new block and wait until it printed the last line.
fn printed(env: &Env, n: usize) -> String {
    let b = env.ok(&["run", "--", "sh", "-c", &format!("seq 1 {n}; exec sleep 300")]);
    let last = n.to_string();
    let end = Instant::now() + Duration::from_secs(20);
    while !env.capture(&b).lines().any(|l| l.trim() == last) {
        assert!(Instant::now() < end, "never printed {n}");
        std::thread::sleep(Duration::from_millis(50));
    }
    b
}

#[test]
fn a_bad_value_is_refused_and_the_file_left_alone() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    const INIT: &str = "-- mine\nleon.set(\"scrollback\", 5000)\n";
    std::fs::write(&path, INIT).unwrap();
    for bad in ["-1", "100001", "1.5", "lots", "true"] {
        let o = config(&env, &path, &["set", "--", "scrollback", bad]);
        assert_eq!(o.status.code(), Some(1), "{bad}: {}", text(&o));
        for word in ["scrollback", "100000"] {
            assert!(text(&o).contains(word), "{bad}: {} should say {word:?}", text(&o));
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), INIT, "{bad} left the file alone");
    }
}

#[test]
fn the_cap_applies_at_once_and_trims_when_lowered() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "-- mine\n").unwrap();
    let o = config(&env, &path, &["get", "scrollback"]);
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "10000", "the default: {}", text(&o));

    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
    let big = printed(&env, 6000);
    let h = history(&env, &big)["history"].as_u64().unwrap();
    assert!((5900..6000).contains(&h), "all of it is kept under the default: {h}");

    let o = config(&env, &path, &["set", "scrollback", "500"]);
    assert!(o.status.success(), "{}", text(&o));
    let r = env.json(&["api", "call", "server.settings"]);
    assert_eq!(r["settings"]["scrollback"], 500, "{r}");
    // the reload trimmed the pane that was already full, without waiting for its output
    assert_eq!(history(&env, &big)["history"], 500, "a lowered cap trims at once");
    let kept = env.capture(&big);
    assert!(!kept.lines().any(|l| l.trim() == "100"), "the oldest lines are gone");
    assert!(kept.lines().any(|l| l.trim() == "5500"), "the newest are kept");
    // a new pane's output is capped from its first line
    let new = printed(&env, 3000);
    assert_eq!(history(&env, &new)["history"], 500, "new lines are capped");

    let o = config(&env, &path, &["set", "scrollback", "20000"]);
    assert!(o.status.success(), "{}", text(&o));
    let more = printed(&env, 15000);
    let h = history(&env, &more)["history"].as_u64().unwrap();
    assert!((14900..15000).contains(&h), "a raised cap keeps more: {h}");
    env.ok(&["server", "stop"]);
}
