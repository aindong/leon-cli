//! Agent orchestration criterion 11: `leon agent tell W TEXT` waits until the worker's turn ended,
//! then types TEXT and presses Enter once; it gives up at `--for`, and refuses a worker waiting on
//! a permission prompt.

mod support;

use std::process::Stdio;
use std::time::Duration;

use support::orch::{until, Orch};

#[test]
fn tell_waits_for_the_worker_to_be_idle_then_types_once() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let reader = o.script("reader-agent", support::orch::READER);
    let w = o.ok_inside(&s, &boss, &["agent", "start", "--label", "w", reader.to_str().unwrap(), "t"]);
    o.env.screen_until(&w, "the worker's prompt", |t| t.contains("ready>"));
    o.env.ok(&["notify", "-b", &w, "--state", "working"]);

    let out = o.run_inside(&s, &boss, &["agent", "tell", "w", "too soon", "--for", "1s"]);
    assert_eq!(out.status.code(), Some(1), "gives up while it works: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(o.count(&w, "GOT: too soon"), 0);

    let mut tell = o
        .inside(&s, &boss)
        .args(["agent", "tell", &w, "use postgres", "--for", "30s"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(o.count(&w, "use postgres"), 0, "nothing typed while it works");
    o.env.ok(&["notify", "-b", &w, "--state", "done", "--body", "which db?"]);
    let status = until("tell to finish", 15, || tell.try_wait().unwrap());
    assert!(status.success());
    o.env.screen_until(&w, "the answer", |t| t.contains("GOT: use postgres"));
    std::thread::sleep(Duration::from_secs(1));
    assert_eq!(o.count(&w, "GOT: use postgres"), 1, "typed once");

    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: ls"]);
    let out = o.run_inside(&s, &boss, &["agent", "tell", &w, "yes", "--for", "5s"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("permission"), "{}", String::from_utf8_lossy(&out.stderr));
}
