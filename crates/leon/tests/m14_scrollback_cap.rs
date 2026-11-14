//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 2): a block's saved
//! scrollback is capped at 2 MiB, keeping the newest lines and dropping the oldest whole lines.

mod support;

use serde_json::Value;
use support::Env;

const CAP: usize = 2 << 20;

#[test]
fn a_big_scrollback_is_saved_capped_newest_lines_kept() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    env.ok(&["-b", &b, "block", "call", "resize", r#"{"rows":50,"cols":1000}"#]);
    // 3000 lines of 1000 characters (~3 MB): line N starts with its 4-digit number.
    let print = "i=0; while [ $i -lt 3000 ]; do printf '%04d%0996d\\n' $i 0; i=$((i+1)); done; echo done-$((1+1))\r";
    env.ok(&["-b", &b, "send", print]);
    env.screen_until(&b, "the output", |s| s.lines().any(|l| l.trim() == "done-2"));

    env.ok(&["server", "stop"]);
    let saved: Value = serde_json::from_slice(&std::fs::read(env.state_dir().join("sessions.json")).unwrap()).unwrap();
    let text = saved["sessions"][0]["blocks"][0]["scrollback"].as_str().expect("a scrollback was saved");
    assert!(text.len() <= CAP, "the snapshot is {} bytes, over the 2 MiB cap", text.len());
    assert!(text.len() > CAP - 2000, "the cap keeps as much as fits: {} bytes", text.len());
    let numbered: Vec<&str> = text.lines().filter(|l| l.len() == 1000).collect();
    assert!(numbered.iter().any(|l| l.starts_with("2999")), "the newest lines are kept");
    assert!(!numbered.iter().any(|l| l.starts_with("0000")), "the oldest lines are dropped");
    let first = text.lines().next().unwrap();
    assert!(first.len() == 1000 && first[..4].bytes().all(|c| c.is_ascii_digit()), "starts on a whole line");

    env.ok(&["server", "start"]);
    env.screen_until(&b, "the restored block", |s| s.contains("— restored from "));
}
