//! Daily-driver F14 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 8): the scratch drawer
//! (`leon toggle scratch --create`, ⌘J in the app) opens where the focused pane is, not in ~.

mod support;

use std::path::{Path, PathBuf};
use support::Env;

fn real(p: &Path) -> PathBuf {
    p.canonicalize().unwrap()
}

/// A session whose one block is a shell that has `cd`'d into `dir`; returns its id.
fn shell_in(env: &Env, name: &str, dir: &Path) -> String {
    let sid = env.ok(&["new", name, "--cwd", "/"]);
    let b = env.window(&sid, 0)["block_ids"][0].as_str().unwrap().to_string();
    env.ok(&["-b", &b, "send", &format!("cd '{}'\r", dir.display())]);
    let want = real(dir);
    env.screen_until(&b, "the shell's cd", |_| {
        let cwd = env.json(&["block", "inspect", &b, "--json"])["cwd"].as_str().map(PathBuf::from);
        cwd.is_some_and(|c| c.canonicalize().is_ok_and(|c| c == want))
    });
    b
}

#[test]
fn the_drawer_opens_in_the_focused_blocks_directory() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let project = env.dir.path().join("src app");
    std::fs::create_dir(&project).unwrap();
    shell_in(&env, "work", &project);

    env.ok(&["-s", "work", "toggle", "scratch", "--create"]);
    let drawer = env.json(&["-s", "work", "block", "inspect", "scratch", "--json"]);
    let cwd = drawer["cwd"].as_str().expect("the drawer has a cwd");
    assert_eq!(real(Path::new(cwd)), real(&project), "the drawer's cwd");
    let id = drawer["block_id"].as_str().unwrap();
    env.ok(&["-b", id, "send", "printf 'DRAWER_PWD=%s\\n' \"$(pwd -P)\"\r"]);
    let want = format!("DRAWER_PWD={}", real(&project).display());
    env.screen_until(id, "pwd in the drawer", |s| s.contains(&want));
}

#[test]
fn a_removed_directory_still_opens_a_drawer() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let gone = env.dir.path().join("gone");
    std::fs::create_dir(&gone).unwrap();
    shell_in(&env, "work", &gone);
    std::fs::remove_dir(&gone).unwrap();

    env.ok(&["-s", "work", "toggle", "scratch", "--create"]);
    let drawer = env.json(&["-s", "work", "block", "inspect", "scratch", "--json"]);
    assert!(drawer["pid"].as_i64().is_some_and(|p| p > 0), "the drawer runs a shell: {drawer}");
}
