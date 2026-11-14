//! M5a criteria 4, 5, 8 (.agent/plans/2026-11-14-m5a-lua-config.md): `leon config check` on the
//! fixtures in tests/fixtures/m5a, black box. It must never touch a server.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Run {
    code: i32,
    out: String,
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/m5a").join(name)
}

/// Run `leon config check ARGS` with a fresh runtime dir, HOME and XDG_CONFIG_HOME, and assert
/// the runtime dir is still empty afterwards (no server was started).
fn check(args: &[&str], env: &[(&str, &Path)]) -> Run {
    let rt = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
    c.args(["config", "check"]).args(args);
    c.env("LEON_RUNTIME_DIR", rt.path())
        .env("HOME", home.path())
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("LEON_CONFIG")
        .env_remove("LEON_AUTOSTART");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().unwrap();
    assert_eq!(std::fs::read_dir(rt.path()).unwrap().count(), 0, "config check touched the runtime dir");
    let out = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
    Run { code: o.status.code().unwrap(), out }
}

fn check_path(name: &str) -> Run {
    check(&[fixture(name).join("init.lua").to_str().unwrap()], &[])
}

#[test]
fn good_config_prints_counts_and_exits_0() {
    let r = check_path("good");
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("8 bindings, 1 mode, 1 action"), "{}", r.out);
    assert!(!r.out.contains("error"), "{}", r.out);
}

#[test]
fn partially_bad_config_skips_bad_calls_and_exits_1() {
    let r = check_path("partial");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("2 bindings, 0 modes, 0 actions"), "{}", r.out);
    for (line, what) in [(2, "hyper"), (3, "Bad.Name"), (4, "mode"), (5, "float")] {
        let hit = r.out.lines().find(|l| l.contains(&format!("init.lua:{line}: error:")));
        let hit = hit.unwrap_or_else(|| panic!("no error for line {line}:\n{}", r.out));
        assert!(hit.contains(what), "line {line} should mention {what:?}: {hit}");
    }
    assert_eq!(r.out.matches(": error:").count(), 4, "{}", r.out);
}

#[test]
fn syntax_error_fails_the_whole_load() {
    let r = check_path("syntax");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("init.lua:3:"), "{}", r.out);
    assert!(!r.out.contains("bindings"), "no counts on a failed load: {}", r.out);
}

#[test]
fn syntax_error_in_a_sibling_fails_the_whole_load() {
    let r = check_path("syntax_in_sibling");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("broken.lua:2:"), "{}", r.out);
    assert!(!r.out.contains("bindings"), "{}", r.out);
}

#[test]
fn runtime_error_fails_the_whole_load() {
    let r = check_path("runtime_error");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("init.lua:3:") && r.out.contains("index"), "{}", r.out);
    assert!(!r.out.contains("bindings"), "{}", r.out);
}

#[test]
fn require_loads_siblings() {
    let r = check_path("require");
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("3 bindings, 0 modes, 0 actions"), "{}", r.out);
}

#[test]
fn bad_call_in_a_sibling_reports_the_siblings_line() {
    let r = check_path("sibling_bad");
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("3 bindings"), "{}", r.out);
    let errs: Vec<_> = r.out.lines().filter(|l| l.contains(": error:")).collect();
    assert_eq!(errs.len(), 1, "{}", r.out);
    assert!(errs[0].contains("sub/bad.lua:3: error:"), "{}", errs[0]);
}

#[test]
fn missing_file_exits_1() {
    let r = check(&["/nonexistent/leon/init.lua"], &[]);
    assert_eq!(r.code, 1, "{}", r.out);
    assert!(r.out.contains("/nonexistent/leon/init.lua"), "{}", r.out);
}

#[test]
fn default_path_follows_leon_config_then_xdg_then_home() {
    let good = fixture("good").join("init.lua");
    let partial = fixture("partial");
    let require = fixture("require");

    // LEON_CONFIG wins over XDG_CONFIG_HOME
    let xdg = tempfile::tempdir().unwrap();
    copy_dir(&partial, &xdg.path().join("leon"));
    let r = check(&[], &[("LEON_CONFIG", &good), ("XDG_CONFIG_HOME", xdg.path())]);
    assert!(r.out.contains("8 bindings"), "{}", r.out);

    // then $XDG_CONFIG_HOME/leon/init.lua
    let r = check(&[], &[("XDG_CONFIG_HOME", xdg.path())]);
    assert!(r.out.contains("2 bindings"), "{}", r.out);

    // then ~/.config/leon/init.lua
    let home = tempfile::tempdir().unwrap();
    copy_dir(&require, &home.path().join(".config/leon"));
    let r = check(&[], &[("HOME", home.path())]);
    assert_eq!(r.code, 0, "{}", r.out);
    assert!(r.out.contains("3 bindings"), "{}", r.out);
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}
