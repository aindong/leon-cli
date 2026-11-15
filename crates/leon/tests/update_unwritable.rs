//! Update criterion 10 (.agent/plans/2026-11-16-self-update-core.md): leon starts no install it
//! cannot finish. A destination it cannot write is an error that names the path, before any
//! download, and it never reaches for `sudo`.

mod release_kit;
mod update_kit;

use std::os::unix::fs::PermissionsExt;

use update_kit::{assert_refused, name, Install, Release};

fn set_mode(dir: &std::path::Path, mode: u32) {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn a_destination_that_is_not_writable_is_named_and_nothing_is_downloaded() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let dir = leon.exe.parent().unwrap().to_path_buf();
    let before = leon.state();
    set_mode(&dir, 0o555);
    let o = leon.update(&rel, &srv, &[]);
    set_mode(&dir, 0o755); // so the assertions below can look, and the temp dir can go
    assert_refused(&o, &["can't write", dir.to_str().unwrap()]);
    leon.assert_unchanged(before, "a destination that is not writable");
    assert!(!srv.asked().contains(&name("cli")), "nothing was downloaded: {:?}", srv.asked());
    assert!(!leon.sudo_ran(), "leon never runs sudo");
    // --check installs nothing, so it has nothing to refuse
    set_mode(&dir, 0o555);
    let o = leon.update(&rel, &srv, &["--check"]);
    set_mode(&dir, 0o755);
    assert_eq!(o.code, 10, "--check still reports the release: {}", o.text());
}

// macOS only: the app kind is a zip of Leon.app, and only macOS has it
#[cfg(target_os = "macos")]
#[test]
fn an_app_that_is_not_this_users_to_move_is_refused_before_the_download() {
    // as an app that root or another admin put in the group-writable /Applications
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::app());
    let before = leon.state();
    set_mode(&leon.root, 0o555);
    let o = leon.update(&rel, &srv, &[]);
    set_mode(&leon.root, 0o755);
    assert_refused(&o, &["can't write", leon.root.to_str().unwrap()]);
    leon.assert_unchanged(before, "an app that cannot be moved");
    assert!(!srv.asked().contains(&name("app")), "nothing was downloaded: {:?}", srv.asked());
    assert!(!leon.sudo_ran(), "leon never runs sudo");
}

#[test]
fn a_writable_destination_never_needs_sudo_either() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert!(!leon.sudo_ran(), "leon never runs sudo");
}
