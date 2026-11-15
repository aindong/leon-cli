//! Update criterion 22 (.agent/plans/2026-11-16-self-update-core.md): an update works in
//! `.leon-update` beside the install, a private directory it holds a lock on. A second update of
//! the same install is turned away, and anything else of that name is left alone.

mod release_kit;
mod update_kit;

use std::os::unix::fs::PermissionsExt;

use update_kit::{assert_refused, spawn_until_paused, Install, Release, VERSION};

#[test]
fn a_second_update_of_the_same_install_is_turned_away() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let mut seam = Install::seam(&rel);
    seam["pause"] = "unpacked".into();
    let first = spawn_until_paused(leon.cmd_with(&srv.url(), &seam).arg("self-update"), "unpacked");

    let staging = leon.exe.parent().unwrap().join(".leon-update");
    assert_eq!(leon.leftovers(), std::slice::from_ref(&staging), "one staging directory, beside the install");
    let mode = std::fs::metadata(&staging).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700, "and it is private: {mode:o}");
    let held: Vec<_> = std::fs::read_dir(&staging).unwrap().flatten().map(|e| e.file_name()).collect();
    assert!(!held.is_empty(), "it holds the first update's files");

    let o = leon.update(&rel, &srv, &[]);
    assert_refused(&o, &["another update", staging.to_str().unwrap()]);
    let still: Vec<_> = std::fs::read_dir(&staging).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(still, held, "the first update's files were not touched");
    assert_eq!(leon.state(), before, "and nothing was installed");

    drop(first); // killed
                 // the lock went with the process: the next update takes the directory over
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "{}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
    assert!(leon.leftovers().is_empty(), "{:?}", leon.leftovers());
}

#[test]
fn something_else_named_like_the_staging_directory_is_left_alone() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let staging = leon.exe.parent().unwrap().join(".leon-update");

    // a symlink to someone's directory: nothing in it may be emptied or written
    let theirs = leon.home.join("theirs");
    std::fs::create_dir(&theirs).unwrap();
    std::fs::write(theirs.join("canary"), "keep me").unwrap();
    std::os::unix::fs::symlink(&theirs, &staging).unwrap();
    assert_refused(&leon.update(&rel, &srv, &[]), &[staging.to_str().unwrap(), "in the way"]);
    assert_eq!(std::fs::read_to_string(theirs.join("canary")).unwrap(), "keep me");
    assert_eq!(std::fs::read_dir(&theirs).unwrap().count(), 1, "nothing was written through the symlink");
    assert!(staging.is_symlink(), "and the symlink is still there");
    std::fs::remove_file(&staging).unwrap();

    // a plain file
    std::fs::write(&staging, "keep me").unwrap();
    assert_refused(&leon.update(&rel, &srv, &[]), &[staging.to_str().unwrap(), "in the way"]);
    assert_eq!(std::fs::read_to_string(&staging).unwrap(), "keep me");
    std::fs::remove_file(&staging).unwrap();
    assert_eq!(leon.state(), before, "nothing was installed either time");

    // a directory with a name that only looks alike is not leon's to clear
    let lookalike = leon.exe.parent().unwrap().join(".leon-update-old");
    std::fs::create_dir(&lookalike).unwrap();
    std::fs::write(lookalike.join("canary"), "keep me").unwrap();
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "{}", o.text());
    assert_eq!(std::fs::read_to_string(lookalike.join("canary")).unwrap(), "keep me");
}
