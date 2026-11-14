//! M14 acceptance (.agent/plans/2026-11-15-m14-resume-after-restart.md, criteria 1 and 6): a block's
//! text survives `server stop` + `server start`, shown above a dim "— restored from <time> —" line
//! and a fresh shell, never re-run; the state file stays private.

mod support;

use std::os::unix::fs::PermissionsExt;

use serde_json::Value;
use support::Env;

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

/// `— restored from YYYY-MM-DD HH:MM —`
fn is_separator(line: &str) -> bool {
    let Some(rest) = line.trim().strip_prefix("— restored from ") else { return false };
    let Some(when) = rest.strip_suffix(" —") else { return false };
    let shape: String = when.chars().map(|c| if c.is_ascii_digit() { '9' } else { c }).collect();
    shape == "9999-99-99 99:99"
}

#[test]
fn a_blocks_text_comes_back_above_a_fresh_shell() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    env.ok(&["-b", &b, "send", "echo before-$((20+1))\r"]);
    env.screen_until(&b, "the echo", |s| s.lines().any(|l| l.trim() == "before-21"));

    env.ok(&["server", "stop"]);
    let file = env.state_dir().join("sessions.json");
    assert_eq!(mode(&file), 0o600, "the state file is private");
    assert_eq!(mode(&env.state_dir()), 0o700, "the state dir is private");
    let names: Vec<String> = std::fs::read_dir(env.state_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["sessions.json"], "the snapshot lives in the state file, no temp file left");
    let saved: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let text = saved["sessions"][0]["blocks"][0]["scrollback"].as_str().unwrap_or_else(|| panic!("{saved}"));
    assert!(text.lines().any(|l| l.trim() == "before-21"), "saved: {text:?}");

    env.ok(&["server", "start"]);
    let s = env.screen_until(&b, "the restored text", |s| s.lines().any(is_separator));
    let lines: Vec<&str> = s.lines().collect();
    let sep = lines.iter().position(|l| is_separator(l)).unwrap();
    let old = lines.iter().position(|l| l.trim() == "before-21").unwrap_or_else(|| panic!("text gone:\n{s}"));
    assert!(old < sep, "the old text is above the separator:\n{s}");

    // The shell below is new and works; the old text was shown, not run again.
    env.ok(&["-b", &b, "send", "echo after-$((20+2))\r"]);
    let s = env.screen_until(&b, "the new shell", |s| s.lines().any(|l| l.trim() == "after-22"));
    let lines: Vec<&str> = s.lines().collect();
    let sep = lines.iter().position(|l| is_separator(l)).unwrap();
    let new = lines.iter().position(|l| l.trim() == "after-22").unwrap();
    assert!(new > sep, "the new output is below the separator:\n{s}");
    assert_eq!(lines.iter().filter(|l| l.trim() == "before-21").count(), 1, "never re-executed:\n{s}");

    // The separator is dim: its colour differs from the plain text's.
    let screen = env.json(&["-b", &b, "block", "call", "screen"]);
    let fg_of = |pred: &dyn Fn(&str) -> bool| {
        screen["rows"].as_array().unwrap().iter().find_map(|r| {
            let text: String = r["spans"].as_array()?.iter().filter_map(|s| s["text"].as_str()).collect();
            pred(&text).then(|| r["spans"][0]["fg"].clone())
        })
    };
    let plain = fg_of(&|t| t.trim() == "before-21").expect("the old text is on screen");
    let dim = fg_of(&|t| is_separator(t)).expect("the separator is on screen");
    assert_ne!(plain, dim, "the separator is drawn dim");
}
