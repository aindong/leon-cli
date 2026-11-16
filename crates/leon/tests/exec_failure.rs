//! A pane whose program can't be executed is refused like a program that isn't there
//! (.agent/plans/2026-11-16-exec-failure.md): `bad_request` naming the program and the OS's
//! reason, exit 1, and no session, window or block left behind. Never std's abort text in a pane.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use support::Env;

fn executable(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// `leon run -- PROGRAM` fails with exit 1 and leaves no session; returns its stderr.
fn refused(env: &Env, program: &Path) -> String {
    let out = env.run(&["run", "--", program.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    assert_eq!(out.status.code(), Some(1), "refused, stdout {:?}, stderr {err}", String::from_utf8_lossy(&out.stdout));
    assert!(err.contains("bad_request") && err.contains(program.to_str().unwrap()), "names the program: {err}");
    assert!(!err.contains("fatal runtime error"), "{err}");
    assert_eq!(env.json(&["ls", "--json"]), serde_json::json!([]), "no session left");
    err
}

#[test]
fn a_missing_interpreter_is_named() {
    let env = Env::new();
    let script = executable(env.dir.path(), "interp", b"#!/no/such/interp\necho hi\n");
    let err = refused(&env, &script);
    assert!(err.contains("/no/such/interp") && err.contains("No such file or directory"), "{err}");
}

#[test]
fn a_file_that_is_not_a_program_is_refused() {
    let env = Env::new();
    let bytes = executable(env.dir.path(), "bytes", b"\x00\x01\x02\xff\xfe not a program\x00\n");
    let err = refused(&env, &bytes);
    assert!(err.contains("Exec format error"), "{err}");
}

#[test]
fn a_directory_is_refused() {
    let env = Env::new();
    let dir = env.dir.path().join("adir");
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let err = refused(&env, &dir);
    assert!(err.contains("is a directory"), "{err}");
}

#[test]
fn a_failed_pane_leaves_the_session_as_it_was() {
    let env = Env::new();
    let (s, _) = env.session();
    let before = env.view(&s);
    let script = executable(env.dir.path(), "interp", b"#!/no/such/interp\n");
    let out = env.run(&["run", "-s", &s, "--", script.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "{err}");
    assert!(err.contains("bad_request") && err.contains("/no/such/interp"), "{err}");
    let after = env.view(&s);
    assert_eq!(after["windows"], before["windows"], "no window or pane added");
    assert_eq!(env.json(&["-s", &s, "block", "ls", "--json"]).as_array().map(Vec::len), Some(1), "no block added");
}

#[test]
fn a_script_without_a_shebang_still_runs() {
    let env = Env::new();
    let script = executable(env.dir.path(), "plain", b"echo ran as sh \"$0\" \"$1\"\nexit 7\n");
    let out = env.run(&["run", "--wait", "--output", "--", script.to_str().unwrap(), "arg1"]);
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(7), "{text} {}", String::from_utf8_lossy(&out.stderr));
    assert!(text.contains(&format!("ran as sh {} arg1", script.display())), "{text}");
}
