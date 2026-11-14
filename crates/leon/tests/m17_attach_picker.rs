//! M17 criterion 3: with no target, `leon attach` lists sessions with a preview, filters as you
//! type, and Enter attaches to the chosen one.

mod support;

use support::host::Host;
use support::Env;

/// The picker's heading, drawn only by the picker.
const PICKER: &str = "Pick a session, tab or block";

#[test]
fn picker_lists_filters_and_attaches() {
    let env = Env::new();
    let a = env.ok(&["run", "--", "sh", "-c", "echo preview-alpha; exec sleep 300"]);
    env.screen_until(&a, "alpha's output", |s| s.contains("preview-alpha"));
    let first = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    env.ok(&["session", "rename", &first, "alpha-one"]);
    env.ok(&["new", "beta-two"]);
    // a new window becomes the session's active tab
    let b = env.ok(&["-s", "beta-two", "run", "--", "sh", "-c", "echo preview-beta; exec sleep 300"]);
    env.screen_until(&b, "beta's output", |s| s.contains("preview-beta"));

    let h = Host::spawn(&env, "exec \"$LEON\" attach", &[]);
    let s =
        h.wait_for("both sessions listed", |s| s.contains(PICKER) && s.contains("alpha-one") && s.contains("beta-two"));
    assert!(s.contains("preview-alpha"), "the first row's block is previewed:\n{s}");

    // fuzzy: the letters in order, not a substring
    h.send(b"btwo");
    h.wait_for("the filter hides alpha", |s| !s.contains("alpha-one") && s.contains("beta-two"));
    h.send(b"\r");
    h.wait_for("attached to beta-two", |s| !s.contains(PICKER) && h.last_row().contains("beta-two"));
    h.wait_text("preview-beta");
    // attached (not previewing), the TUI sizes the pane: 23 rows, a row for the status bar
    Host::until("beta's pane fitted to 23×80", || {
        let v = env.json(&["-b", &b, "block", "call", "size"]);
        (v["rows"].as_u64(), v["cols"].as_u64()) == (Some(23), Some(80))
    });
}
