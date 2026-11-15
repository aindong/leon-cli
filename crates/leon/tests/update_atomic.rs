//! Update criterion 8 (.agent/plans/2026-11-16-self-update-core.md): an update is atomic. Killed
//! in the middle of the download, after unpacking, or just before the rename, the old install
//! still runs; killed after the rename, the new one does. The next update clears what the killed
//! one left. For the CLI (a file renamed over a file) and the app (two directories swapped).

mod release_kit;
mod update_kit;

use std::process::{Child, Stdio};

use serde_json::json;
use update_kit::{name, run, spawn_until_paused, until, Install, Mode, Release, Server, VERSION};

fn install(kind: &str) -> Install {
    match kind {
        "app" => Install::app(),
        _ => Install::cli(),
    }
}

fn kill(mut child: Child) {
    child.kill().unwrap(); // SIGKILL: no handler runs, nothing is tidied
    child.wait().unwrap();
}

/// The killed update left the old leon in place and working; the next one finishes.
fn assert_old_runs_and_next_update_finishes(leon: &Install, rel: &Release, srv: &Server, before: (u64, u64)) {
    assert_eq!(leon.state(), before, "the install is the old one");
    assert!(leon.version().starts_with(&format!("leon {VERSION}")), "the old leon runs: {}", leon.version());
    assert!(!leon.version().contains("(fake)"), "and it is the old binary, not the release's");
    let o = leon.update(rel, srv, &[]);
    assert_eq!(o.code, 0, "the next update goes through: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
    assert!(leon.leftovers().is_empty(), "the killed update's staging directory is gone: {:?}", leon.leftovers());
}

fn killed_in_the_middle_of_the_download(kind: &str) {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), install(kind));
    let before = leon.state();
    srv.set(&name(kind), Mode::Stall);
    let child = leon.cmd(&rel, &srv).arg("self-update").stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap();
    // the download itself, not just the directory: bytes of the archive have reached the disk
    let partial = leon.root.parent().unwrap().join(".leon-update/download");
    until("part of the file to be on disk", 30, || partial.metadata().is_ok_and(|m| m.is_file() && m.len() > 0));
    assert!(partial.metadata().unwrap().len() < rel.read(&name(kind)).len() as u64, "and only part of it");
    kill(child);
    assert!(!leon.leftovers().is_empty(), "the kill left the staging directory, as a real one would");
    srv.clear(&name(kind));
    assert_old_runs_and_next_update_finishes(&leon, &rel, &srv, before);
}

/// Start an update of `rel` that stops at `step` (the debug-only seam), wait until it has, and
/// kill it.
fn kill_at(rel: &Release, kind: &str, step: &str) -> (Server, Install, (u64, u64)) {
    let (srv, leon) = (rel.serve(), install(kind));
    let before = leon.state();
    let mut seam = Install::seam(rel);
    seam["pause"] = step.into();
    let child = spawn_until_paused(leon.cmd_with(&srv.url(), &seam).arg("self-update"), step);
    kill(child);
    assert!(!leon.leftovers().is_empty(), "the kill at {step} left the staging directory");
    (srv, leon, before)
}

fn killed_before_the_rename(kind: &str, step: &str) {
    let rel = Release::new();
    let (srv, leon, before) = kill_at(&rel, kind, step);
    assert_old_runs_and_next_update_finishes(&leon, &rel, &srv, before);
}

/// After the rename the install is the new one, whole; only the cleanup is missing. The release
/// holds a real leon here, because the test goes on to run the leon it installed.
fn killed_after_the_rename(kind: &str) {
    let (rel, new_sum) = Release::real();
    let (srv, leon, before) = kill_at(&rel, kind, "installed");
    assert_ne!(leon.state(), before, "the install was replaced");
    assert_eq!(leon.sum(), new_sum, "by the release's leon");
    assert!(leon.version().starts_with(&format!("leon {VERSION}")), "which runs: {}", leon.version());
    // the new leon is up to date, and its next update still clears what the killed one left
    let after = leon.state();
    let seam = json!({ "keys": rel.key.keys_file, "version": VERSION });
    let o = run(leon.cmd_with(&srv.url(), &seam).arg("self-update"), 60);
    assert_eq!(o.code, 0, "{}", o.text());
    assert!(o.out.contains("up to date"), "{}", o.text());
    assert!(leon.leftovers().is_empty(), "the killed update's staging directory is gone: {:?}", leon.leftovers());
    assert_eq!(leon.state(), after, "and the install is untouched by the cleanup");
}

#[test]
fn cli_killed_in_the_middle_of_the_download() {
    killed_in_the_middle_of_the_download("cli");
}

#[test]
fn cli_killed_after_unpacking() {
    killed_before_the_rename("cli", "unpacked");
}

#[test]
fn cli_killed_just_before_the_rename() {
    killed_before_the_rename("cli", "installing");
}

#[test]
fn cli_killed_after_the_rename() {
    killed_after_the_rename("cli");
}

// macOS only: the app kind is a zip of Leon.app, and only macOS has it
#[cfg(target_os = "macos")]
#[test]
fn app_killed_at_each_point() {
    killed_in_the_middle_of_the_download("app");
    killed_before_the_rename("app", "unpacked");
    killed_before_the_rename("app", "installing");
    killed_after_the_rename("app");
}
