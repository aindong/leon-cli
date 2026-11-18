//! Scrollback B1, criterion 6: restore after a restart saves and brings back the stored lines,
//! the oldest of a full history included, as it did before the store.

mod support;

use serde_json::Value;
use support::Env;

#[test]
fn stored_lines_are_saved_and_restored() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "sh", "-c", "seq 1 6000 | sed 's/^/kept-/'; exec sleep 300"]);
    env.screen_until(&b, "the last line", |s| s.lines().any(|l| l.trim() == "kept-6000"));
    let before = env.capture(&b);
    assert!(before.lines().any(|l| l == "kept-1"), "the oldest line is kept before the restart");

    env.ok(&["server", "stop"]);
    let saved: Value = serde_json::from_slice(&std::fs::read(env.state_dir().join("sessions.json")).unwrap()).unwrap();
    let text = saved["sessions"][0]["blocks"][0]["scrollback"].as_str().unwrap_or_else(|| panic!("{saved}"));
    let kept: Vec<&str> = text.lines().filter(|l| l.starts_with("kept-")).collect();
    let want: Vec<String> = (1..=6000).map(|n| format!("kept-{n}")).collect();
    assert_eq!(kept, want, "every line was saved, the store's oldest first");

    env.ok(&["server", "start"]);
    let after = env.screen_until(&b, "the restored text", |s| s.contains("— restored from"));
    let back: Vec<&str> = after.lines().filter(|l| l.starts_with("kept-")).collect();
    assert_eq!(back, want, "every line came back, in order");
    env.ok(&["server", "stop"]);
}
