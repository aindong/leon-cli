//! The permission guard, after a pane closes (docs/architecture/attention.md): a process that
//! outlives its pane (a job in a process group of its own that ignores the hangup) still counts as
//! inside leon, so it can't type into a block held for a permission prompt.

mod support;

use std::time::{Duration, Instant};

use support::orch::Orch;

#[test]
fn a_process_from_a_closed_pane_still_counts_as_inside() {
    let o = Orch::new();
    let w = o.env.ok(&[
        "run",
        "--label",
        "worker",
        "--",
        "sh",
        "-c",
        "echo ready; while IFS= read -r l; do echo \"GOT: $l\"; done",
    ]);
    o.env.screen_until(&w, "the worker", |t| t.contains("ready"));
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);

    let dir = o.env.dir.path();
    let (go, code) = (dir.join("go"), dir.join("code"));
    // a job with job control on (its own process group) that ignores SIGHUP, so closing the pane
    // leaves it running in the pane's session; once told, it tries to answer the prompt
    let job = format!(
        "trap '' HUP; while [ ! -e '{go}' ]; do sleep 0.1; done; leon send -b {w} yes; echo \\$? > '{code}'",
        go = go.display(),
        code = code.display()
    );
    let pane = o.script("outliver", &format!("set -m\nsh -c \"{job}\" &\necho started; exec sleep 300"));
    let p = o.env.ok(&["run", "--label", "outliver", "--", pane.to_str().unwrap()]);
    o.env.screen_until(&p, "the job", |t| t.contains("started"));
    o.env.ok(&["block", "close", &p]);
    std::fs::write(&go, "").unwrap();

    let end = Instant::now() + Duration::from_secs(10);
    while !code.exists() {
        assert!(Instant::now() < end, "the job never tried");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(100)); // the exit code's write lands
    let code = std::fs::read_to_string(&code).unwrap();
    assert_ne!(code.trim(), "0", "OUTSIDE: a process from a closed pane typed into the held prompt");
    assert_eq!(o.count(&w, "GOT:"), 0, "nothing reached the prompt:\n{}", o.env.capture(&w));
}
