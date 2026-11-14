//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criterion 3): a saved snapshot
//! with escape sequences in it (the state file edited by hand) restores as plain text: no colour, no
//! title, no reply typed into the shell, no carriage return.

mod support;

use serde_json::Value;
use support::Env;

#[test]
fn escapes_in_a_saved_snapshot_restore_inert() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    env.ok(&["server", "stop"]);

    let file = env.state_dir().join("sessions.json");
    let mut saved: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let evil = "plain-start\n\
                \u{1b}[31mRED\u{1b}[0m\n\
                \u{1b}]2;pwned-title\u{7}\n\
                ask:\u{1b}[6n\n\
                c1:\u{9b}31mX\n\
                over\rwrite\n\
                plain-end";
    saved["sessions"][0]["blocks"][0]["scrollback"] = Value::from(evil);
    std::fs::write(&file, serde_json::to_vec(&saved).unwrap()).unwrap();

    env.ok(&["server", "start"]);
    let s = env.screen_until(&b, "the restored snapshot", |s| s.contains("— restored from "));
    let lines: Vec<&str> = s.lines().map(str::trim_end).collect();
    for want in ["plain-start", "[31mRED[0m", "]2;pwned-title", "ask:[6n", "c1:31mX", "overwrite", "plain-end"] {
        assert!(lines.contains(&want), "{want:?} shows as plain text:\n{s}");
    }

    // Nothing was typed into the new shell (a cursor-position reply would be): a command runs clean.
    env.ok(&["-b", &b, "send", "echo clean-$((1+1))\r"]);
    env.screen_until(&b, "a clean prompt", |s| s.lines().any(|l| l.trim() == "clean-2"));

    // No colour reached the screen: the snapshot's rows all have the plain text's colour.
    let screen = env.json(&["-b", &b, "block", "call", "screen"]);
    let rows = screen["rows"].as_array().unwrap();
    let text =
        |r: &Value| -> String { r["spans"].as_array().unwrap().iter().filter_map(|s| s["text"].as_str()).collect() };
    let plain = rows.iter().find(|r| text(r).trim() == "plain-start").expect("on screen")["spans"][0]["fg"].clone();
    let red = rows.iter().find(|r| text(r).contains("RED")).expect("on screen");
    for span in red["spans"].as_array().unwrap() {
        assert_eq!(span["fg"], plain, "no colour from the snapshot: {red}");
    }
}
