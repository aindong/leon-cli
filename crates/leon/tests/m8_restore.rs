//! M8 acceptance (.agent/plans/2026-11-14-m8-persist-speed.md, criteria 1, 5, 8): `server stop` then
//! `server start` brings back sessions, windows, layout trees, labels, focus and each shell's cwd,
//! with fresh login shells; the state file is private.

mod support;

use std::os::unix::fs::PermissionsExt;

use serde_json::Value;
use support::Env;

fn mode(p: &std::path::Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

/// A view without its revision, which may grow across a restart.
fn without_revision(mut v: Value) -> (Value, u64) {
    let r = v["revision"].as_u64().unwrap_or_else(|| panic!("no revision in {v}"));
    v.as_object_mut().unwrap().remove("revision");
    (v, r)
}

/// The fields of each block that must come back (pid and command belong to the new program).
fn blocks(env: &Env, sid: &str) -> Vec<Value> {
    let all = env.json(&["-s", sid, "block", "ls", "--json"]);
    let keep = |b: &Value| serde_json::json!([b["block_id"], b["label"], b["window_id"], b["keep_open"]]);
    all.as_array().unwrap().iter().map(keep).collect()
}

#[test]
fn stop_and_start_bring_back_sessions_layout_labels_and_cwd() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let work = env.dir.path().join("work");
    std::fs::create_dir_all(work.join("sub")).unwrap();
    let sub = std::fs::canonicalize(work.join("sub")).unwrap();
    let work = work.to_str().unwrap();

    // web: window 1 a login shell; window 2 "editor" (active) = main | side at 30%, main focused and zoomed.
    let web = env.ok(&["new", "web"]);
    let main = env.ok(&["-s", "web", "run", "--label", "main", "-c", work, "--", "/bin/sh"]);
    env.ok(&["-s", "web", "-b", &main, "split", "--split", "right", "--ratio", "30", "--label", "side", "-c", work]);
    env.ok(&["-s", "web", "focus", "main"]);
    env.ok(&["-s", "web", "zoom", "main", "--on"]);
    env.ok(&["-s", "web", "window", "rename", "2", "editor"]);
    // api: window 1 (made active again) and window 2 "logs" running a program, not a shell.
    let api = env.ok(&["new", "api"]);
    let first = env.json(&["-s", "api", "block", "ls", "--json"])[0]["block_id"].as_str().unwrap().to_string();
    let sleeper = env.ok(&["-s", "api", "run", "--label", "sleeper", "--keep-open", "--", "sleep", "300"]);
    env.ok(&["-s", "api", "window", "rename", "2", "logs"]);
    env.ok(&["-s", "api", "focus", &first]);

    env.ok(&["-s", "web", "-b", "main", "send", "cd sub; echo cd-$((40+2))\r"]);
    env.screen_until(&main, "the cd", |s| s.lines().any(|l| l.trim() == "cd-42"));

    let ls = env.json(&["ls", "--json"]);
    let views: Vec<(Value, u64)> = [&web, &api].iter().map(|s| without_revision(env.view(s))).collect();
    let block_lists: Vec<Vec<Value>> = [&web, &api].iter().map(|s| blocks(&env, s)).collect();
    assert_eq!(views[0].0["windows"][1]["zoomed_block_id"], main.as_str(), "setup: main is zoomed");
    assert_eq!(views[1].0["windows"][0]["active"], true, "setup: api's first window is active");

    env.ok(&["server", "stop"]);
    let file = env.state_dir().join("sessions.json");
    assert!(file.is_file(), "server stop saved nothing at {}", file.display());
    assert_eq!(mode(&file), 0o600, "the state file is private");
    assert_eq!(mode(&env.state_dir()), 0o700, "the state dir is private");
    let names: Vec<String> = std::fs::read_dir(env.state_dir())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["sessions.json"], "no temp file is left beside the state file");

    env.ok(&["server", "start"]);
    assert_eq!(env.json(&["ls", "--json"]), ls, "the same sessions, in order, with labels and window counts");
    for (i, sid) in [&web, &api].iter().enumerate() {
        let (view, revision) = without_revision(env.view(sid));
        assert_eq!(view, views[i].0, "session {sid}: windows, layout tree, ratios, focus and zoom come back");
        assert!(revision >= views[i].1, "revision never goes backwards");
        assert_eq!(blocks(&env, sid), block_lists[i], "session {sid}: block ids, labels, windows, keep-open");
    }

    // Programs don't survive: every restored block runs a fresh login shell, the sleeper too.
    let inspected = env.json(&["-s", "api", "block", "inspect", &sleeper, "--json"]);
    assert_eq!(inspected["command"], serde_json::json!(["/bin/sh"]), "a saved command is never re-run");

    env.ok(&["-s", "web", "-b", "main", "send", "pwd\r"]);
    let want = sub.to_str().unwrap();
    env.screen_until(&main, "pwd in the restored shell", |s| s.lines().any(|l| l.trim() == want));
}
