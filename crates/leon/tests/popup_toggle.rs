//! Popups (.agent/plans/2026-11-17-popup-toggle.md): `leon toggle LABEL --create --float -- CMD`
//! opens a float running CMD where the focused pane is; the same toggle hides and shows it; it
//! closes when CMD exits, giving the focus back.

mod support;

use serde_json::Value;
use std::path::{Path, PathBuf};
use support::host::{attach_then_stty, Host};
use support::Env;

fn real(p: &Path) -> PathBuf {
    p.canonicalize().unwrap()
}

/// A session of two tiles, the second focused, both running `sleep 300` in `dir`; returns
/// (session, first tile, focused tile).
fn two_tiles(e: &Env, dir: &Path) -> (String, String, String) {
    let first = e.ok(&["run", "--cwd", dir.to_str().unwrap(), "--", "sleep", "300"]);
    let sid = e.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let second = e.ok(&["-b", &first, "split", "--cwd", dir.to_str().unwrap(), "--", "sleep", "300"]);
    assert_eq!(e.window(&sid, 0)["focused_block_id"], second.as_str());
    (sid, first, second)
}

fn inspect(e: &Env, sid: &str, label: &str) -> Value {
    e.json(&["-s", sid, "block", "inspect", label, "--json"])
}

fn floats(e: &Env, sid: &str) -> Vec<Value> {
    e.window(sid, 0)["floating"].as_array().cloned().unwrap_or_default()
}

fn until(what: &str, pred: impl Fn() -> bool) {
    Host::until(what, pred)
}

#[test]
fn a_popup_runs_its_command_where_the_focused_pane_is_and_toggles() {
    let e = Env::new();
    let project = e.dir.path().join("my project");
    std::fs::create_dir(&project).unwrap();
    let (sid, _, before) = two_tiles(&e, &project);
    // a hidden scratch drawer (the app's ⌘J) doesn't stop a float
    e.ok(&["-s", &sid, "toggle", "scratch", "--create"]);
    e.ok(&["-s", &sid, "toggle", "scratch"]);
    assert_eq!(e.window(&sid, 0)["focused_block_id"], before.as_str());

    e.ok(&["-s", &sid, "toggle", "git", "--create", "--float", "--", "sh", "-c", "pwd -P; exec sleep 300"]);
    let w = e.window(&sid, 0);
    assert_eq!(w["docked"].as_array().unwrap().len(), 1, "no second drawer: {w}");
    let pop = inspect(&e, &sid, "git");
    let id = pop["block_id"].as_str().unwrap().to_string();
    assert_eq!(w["floating"][0]["block_id"], id.as_str());
    assert_eq!(w["floating"][0]["hidden"], false);
    let b = &w["floating"][0]["bounds"];
    assert_eq!((b["x"].as_f64(), b["width"].as_f64()), (Some(0.1), Some(0.8)), "the float default: {b}");
    assert_eq!(w["focused_block_id"], id.as_str(), "opening focuses the popup");
    let want = real(&project).display().to_string();
    e.screen_until(&id, "the popup's pwd", |s| s.contains(&want));

    e.ok(&["-s", &sid, "toggle", "git"]);
    let w = e.window(&sid, 0);
    assert_eq!(w["floating"][0]["hidden"], true);
    assert_eq!(w["focused_block_id"], before.as_str(), "hiding gives the focus back");
    assert!(inspect(&e, &sid, "git")["exit_code"].is_null(), "the hidden program keeps running");

    e.ok(&["-s", &sid, "toggle", "git"]);
    let w = e.window(&sid, 0);
    assert_eq!(w["floating"][0]["hidden"], false);
    assert_eq!(w["focused_block_id"], id.as_str());
}

#[test]
fn a_popup_closes_when_its_program_exits_and_the_next_toggle_makes_a_new_one() {
    let e = Env::new();
    let (sid, _, before) = two_tiles(&e, e.dir.path());
    let create = ["-s", &sid, "toggle", "pop", "--create", "--float", "--", "sh", "-c", "read x"];
    e.ok(&create);
    let id = inspect(&e, &sid, "pop")["block_id"].as_str().unwrap().to_string();
    e.ok(&["-b", &id, "send", "\r"]);
    until("the popup closes", || floats(&e, &sid).is_empty());
    assert_eq!(e.window(&sid, 0)["focused_block_id"], before.as_str(), "focus goes back");

    e.ok(&create);
    let again = floats(&e, &sid);
    assert_eq!(again.len(), 1);
    assert_ne!(again[0]["block_id"], id.as_str(), "a new popup");
}

#[test]
fn keep_open_keeps_the_popup_with_its_exit_status() {
    let e = Env::new();
    let (sid, _) = e.session();
    e.ok(&["-s", &sid, "toggle", "done", "--create", "--float", "--keep-open", "--", "sh", "-c", "exit 3"]);
    until("the program exits", || inspect(&e, &sid, "done")["exit_code"] == 3);
    assert_eq!(floats(&e, &sid).len(), 1);
}

#[test]
fn rect_sets_the_bounds_and_bad_input_creates_nothing() {
    let e = Env::new();
    let (sid, _) = e.session();
    e.ok(&["-s", &sid, "toggle", "r", "--create", "--float", "--rect", "0.2,0.2,0.6,0.6", "--", "sleep", "300"]);
    let b = &floats(&e, &sid)[0]["bounds"];
    assert_eq!((b["x"].as_f64(), b["height"].as_f64()), (Some(0.2), Some(0.6)));

    let bad = ["-s", &sid, "toggle", "bad", "--create", "--float", "--rect", "0.5,0.5,0.9,0.9", "--", "sleep", "300"];
    assert!(!e.run(&bad).status.success());
    for args in [
        &["-s", &sid, "toggle", "x", "--float"][..],
        &["-s", &sid, "toggle", "x", "--rect", "0.1,0.1,0.5,0.5"],
        &["-s", &sid, "toggle", "x", "--keep-open"],
        &["-s", &sid, "toggle", "x", "--", "sleep", "300"],
    ] {
        assert!(!e.run(args).status.success(), "refused: {args:?}");
    }
    assert_eq!(floats(&e, &sid).len(), 1, "nothing else was made");
    assert_eq!(e.view(&sid)["windows"][0]["layout"].to_string().matches("block:").count(), 1);
}

#[test]
fn without_float_a_command_runs_in_the_bottom_drawer() {
    let e = Env::new();
    let (sid, _) = e.session();
    e.ok(&["-s", &sid, "toggle", "logs", "--create", "--", "sh", "-c", "echo DRAWER_CMD; exec sleep 300"]);
    let w = e.window(&sid, 0);
    assert_eq!(w["docked"][0]["side"], "bottom");
    assert!(floats(&e, &sid).is_empty());
    let id = w["docked"][0]["block_id"].as_str().unwrap().to_string();
    e.screen_until(&id, "the drawer's command", |s| s.contains("DRAWER_CMD"));
}

#[test]
fn attach_draws_the_popup() {
    let e = Env::new();
    let (sid, _) = e.session();
    e.ok(&["session", "rename", &sid, "popattach"]);
    e.ok(&["-s", &sid, "toggle", "git", "--create", "--float", "--", "sh", "-c", "echo POPUP_MARK; exec sleep 300"]);
    let h = Host::spawn(&e, &attach_then_stty("popattach"), &[]);
    let frame = h.wait_text("POPUP_MARK");
    assert!(frame.contains('┌') && frame.contains('┘'), "float border: {frame}");
    h.send(b"\x02d");
}

#[test]
fn the_git_popup_recipe_toggles_on_its_key() {
    let e = Env::new();
    // stand-ins for git and the pager, so the recipe runs the same on every CI machine
    let bin = e.dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    for (name, body) in [("git", "echo FAKE_GIT_LOG"), ("less", "cat; exec sleep 300")] {
        let p = bin.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    assert!(e.cmd().env("PATH", &path).args(["server", "start"]).status().unwrap().success());
    let (sid, _) = e.session();

    let docs = include_str!("../../../docs/lua/keys.md");
    let recipe = docs
        .split("<!-- popup-git -->")
        .nth(1)
        .expect("the recipe's marker")
        .split("```lua\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    assert!(recipe.contains("\"ctrl+alt+g\""), "the key the test presses: {recipe}");
    std::fs::write(e.dir.path().join("no-init.lua"), recipe).unwrap();
    e.ok(&["config", "reload"]);
    let params = serde_json::json!({"session_id": sid, "key": "ctrl+alt+g"}).to_string();
    e.ok(&["api", "call", "client.key", &params]);
    let pop = inspect(&e, &sid, "git");
    let id = pop["block_id"].as_str().unwrap().to_string();
    assert_eq!(floats(&e, &sid)[0]["block_id"], id.as_str());
    e.screen_until(&id, "the git log in the pager", |s| s.contains("FAKE_GIT_LOG"));
    e.ok(&["api", "call", "client.key", &params]);
    assert_eq!(floats(&e, &sid)[0]["hidden"], true);
}
