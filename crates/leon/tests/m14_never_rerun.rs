//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 5): a plain
//! `leon run -- <cmd>` block, with no resume command, is never re-run on restore, even with the
//! resume setting at "always".

mod support;

use std::time::{Duration, Instant};

use support::Env;

#[test]
fn a_saved_command_is_never_run_again() {
    let env = Env::new();
    let cfg = env.dir.path().join("init.lua");
    std::fs::write(&cfg, "leon.set(\"resume\", \"always\")\n").unwrap();
    let start = || {
        let o = env.cmd().env("LEON_CONFIG", &cfg).args(["server", "start"]).output().unwrap();
        assert!(o.status.success(), "server start: {}", String::from_utf8_lossy(&o.stderr));
    };
    start();
    let mark = env.dir.path().join("deployed");
    let deploy = format!("echo deploying; echo deployed >> '{}'; sleep 300", mark.display());
    let b = env.ok(&["run", "--", "sh", "-c", &deploy]);
    let end = Instant::now() + Duration::from_secs(10);
    while !mark.exists() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(mark.exists(), "the command ran once");

    env.ok(&["server", "stop"]);
    start();
    // Wait until the restored shell is up and answering, so a re-run would have happened by now.
    env.screen_until(&b, "the restored block", |s| s.contains("— restored from "));
    env.ok(&["-b", &b, "send", "echo probe-$((1+1))\r"]);
    env.screen_until(&b, "the new shell", |s| s.lines().any(|l| l.trim() == "probe-2"));

    assert_eq!(std::fs::read_to_string(&mark).unwrap(), "deployed\n", "run exactly once");
    let v = env.json(&["block", "inspect", &b, "--json"]);
    assert_eq!(v["resume"], serde_json::Value::Null, "nothing registered: {v}");
    assert_eq!(v["command"], serde_json::json!(["/bin/sh"]), "a login shell runs instead: {v}");
}
