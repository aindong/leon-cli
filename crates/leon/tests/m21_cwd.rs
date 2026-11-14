//! M21: the first terminal starts in the caller's chosen directory.
mod support;

use serde_json::json;
use std::path::Path;
use support::Env;

fn assert_cwd(env: &Env, session: &str, expected: &Path) {
    let view = env.window(session, 0);
    let block = view["block_ids"][0].as_str().unwrap();
    let got = env.json(&["block", "inspect", block, "--json"]);
    assert_eq!(Path::new(got["cwd"].as_str().unwrap()).canonicalize().unwrap(), expected.canonicalize().unwrap());
    env.ok(&["send", "-b", block, "printf 'M21_PWD='; pwd\n"]);
    let want = format!("M21_PWD={}", expected.canonicalize().unwrap().display());
    env.screen_until(block, "the shell's actual cwd", |s| s.contains(&want));
}

#[test]
fn absolute_and_relative_cwd_use_the_callers_directory() {
    let env = Env::new();
    env.ok(&["server", "start"]); // already running in a different cwd
    let project = env.dir.path().join("project space");
    std::fs::create_dir(&project).unwrap();
    for (prefix, cwd) in [(vec!["new"], project.to_str().unwrap()), (vec!["session", "new"], "project space")] {
        let out = env.cmd().current_dir(env.dir.path()).args(prefix).args(["--cwd", cwd]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_cwd(&env, String::from_utf8_lossy(&out.stdout).trim(), &project);
    }
}

#[test]
fn missing_and_non_directory_are_usage_errors_without_a_session() {
    let env = Env::new();
    let file = env.dir.path().join("file");
    std::fs::write(&file, "not a directory").unwrap();
    for path in [env.dir.path().join("missing"), file] {
        let out = env.run(&["new", "--cwd", path.to_str().unwrap()]);
        assert_eq!(out.status.code(), Some(2));
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(err.contains("directory") && err.contains(path.to_str().unwrap()), "{err}");
    }
    assert_eq!(env.json(&["ls", "--json"]), json!([]));
}

#[test]
fn server_create_cwd_is_additive_and_invalid_paths_leave_no_session() {
    let env = Env::new();
    let dir = env.dir.path().canonicalize().unwrap();
    let p = json!({"label":"api", "cwd":dir}).to_string();
    let r = env.json(&["api", "call", "session.create", &p]);
    assert_cwd(&env, r["session_id"].as_str().unwrap(), &dir);
    let p = json!({"cwd":dir.join("missing")}).to_string();
    let out = env.run(&["api", "call", "session.create", &p]);
    assert!(!out.status.success());
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 1);
}
