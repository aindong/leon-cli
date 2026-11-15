//! Update criterion 15 (.agent/plans/2026-11-16-self-update-core.md): a leon that Homebrew or dpkg
//! installed is theirs to update. `leon self-update` changes nothing and prints the command. The
//! prefixes are fake and so is `dpkg`: no test runs a real package manager.

mod release_kit;
mod update_kit;

use std::os::unix::fs::PermissionsExt;

use serde_json::json;
use update_kit::{name_for, run, Install, Release, Server, VERSION};

const FORMULA: &str = "brew upgrade aindong/leon/leon";
const CASK: &str = "brew upgrade --cask aindong/leon/leon";

fn assert_deferred(leon: &Install, rel: &Release, srv: &Server, env: &[(&str, &str)], says: &str) {
    let before = leon.state();
    let o = run(leon.cmd(rel, srv).envs(env.iter().copied()).arg("self-update"), 60);
    assert_eq!(o.code, 0, "deferring is not a failure: {}", o.text());
    assert!(o.out.contains(&format!("leon {VERSION} is available")), "it still reports the release: {}", o.text());
    assert!(o.out.contains(says), "it prints {says:?}: {}", o.text());
    leon.assert_unchanged(before, says);
    assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no file was fetched: {:?}", srv.asked());
    let o = run(leon.cmd(rel, srv).envs(env.iter().copied()).args(["self-update", "--check"]), 60);
    assert_eq!(o.code, 10, "--check keeps its status: {}", o.text());
    assert!(o.out.contains(says), "--check prints {says:?} too: {}", o.text());
}

#[test]
fn a_homebrew_formula_install_prints_brew_upgrade() {
    let rel = Release::new();
    let leon = Install::at("brew/Cellar/leon/0.0.1/bin/leon");
    // what is on PATH is Homebrew's symlink; the real path is what counts
    std::fs::create_dir(leon.home.join("brew/bin")).unwrap();
    let link = leon.home.join("brew/bin/leon");
    std::os::unix::fs::symlink(&leon.exe, &link).unwrap();
    let srv = rel.serve();
    assert_deferred(&leon, &rel, &srv, &[], FORMULA);
    let o = run(leon.cmd(&rel, &srv).arg("self-update").arg0_path(&link), 60);
    assert!(o.out.contains(FORMULA) && !o.out.contains("--cask"), "through the symlink too: {}", o.text());
}

/// `Command::new(link)` with the rest of a prepared command's setup.
trait Arg0 {
    fn arg0_path(&mut self, path: &std::path::Path) -> &mut Self;
}

impl Arg0 for std::process::Command {
    fn arg0_path(&mut self, path: &std::path::Path) -> &mut Self {
        let mut via = std::process::Command::new(path);
        via.args(self.get_args());
        for (k, v) in self.get_envs() {
            match v {
                Some(v) => via.env(k, v),
                None => via.env_remove(k),
            };
        }
        *self = via;
        self
    }
}

#[test]
fn a_homebrew_cask_install_prints_brew_upgrade_cask() {
    // Homebrew moves a cask's app to /Applications and leaves a symlink in its Caskroom
    let rel = Release::new();
    let leon = Install::app();
    let room = leon.home.join("brew/Caskroom/leon/0.0.1");
    std::fs::create_dir_all(&room).unwrap();
    std::os::unix::fs::symlink(&leon.root, room.join("Leon.app")).unwrap();
    let prefix = leon.home.join("brew");
    assert_deferred(&leon, &rel, &rel.serve(), &[("HOMEBREW_PREFIX", prefix.to_str().unwrap())], CASK);
    // and an app that stayed inside the Caskroom
    let leon = Install::at("brew/Caskroom/leon/0.0.1/Leon.app/Contents/MacOS/leon");
    assert_deferred(&leon, &rel, &rel.serve(), &[], CASK);
}

#[test]
fn an_app_that_another_caskroom_entry_points_past_is_not_homebrews() {
    let rel = Release::new();
    let leon = Install::app();
    let room = leon.home.join("brew/Caskroom/leon/0.0.1");
    std::fs::create_dir_all(&room).unwrap();
    std::fs::create_dir_all(leon.home.join("elsewhere/Leon.app")).unwrap();
    std::os::unix::fs::symlink(leon.home.join("elsewhere/Leon.app"), room.join("Leon.app")).unwrap();
    let prefix = leon.home.join("brew");
    let o = run(leon.cmd(&rel, &rel.serve()).env("HOMEBREW_PREFIX", &prefix).arg("self-update"), 60);
    assert!(!o.text().contains("brew"), "this app is not the cask's: {}", o.text());
    // so leon treats it as its own: it updates it where a release has an app, and looks for one where none does
    if cfg!(target_os = "macos") {
        assert_eq!(o.code, 0, "{}", o.text());
        assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
    } else {
        update_kit::assert_refused(&o, &["has no app file for linux"]);
    }
}

#[test]
fn a_dpkg_install_prints_the_debs_address() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::at("usr/bin/leon"));
    // a dpkg that owns whatever it is asked about, and writes down what that was
    let dpkg = leon.fakebin().join("dpkg");
    std::fs::write(&dpkg, "#!/bin/sh\necho \"$@\" >> \"$(dirname \"$0\")/dpkg-asked\"\necho \"leon: $2\"\n").unwrap();
    std::fs::set_permissions(&dpkg, std::fs::Permissions::from_mode(0o755)).unwrap();
    let deb = format!("{}/{}", srv.url(), name_for("deb", "linux"));
    assert_deferred(&leon, &rel, &srv, &[], &deb);
    let asked = std::fs::read_to_string(leon.fakebin().join("dpkg-asked")).unwrap();
    assert!(asked.contains(&format!("-S {}", leon.exe.display())), "dpkg was asked who owns the binary: {asked}");
}

#[test]
fn up_to_date_says_so_whoever_installed_it() {
    let rel = Release::new();
    let leon = Install::at("brew/Cellar/leon/0.0.1/bin/leon");
    let seam = json!({ "keys": rel.key.keys_file, "version": VERSION });
    let o = run(leon.cmd_with(&rel.serve().url(), &seam).arg("self-update"), 60);
    assert_eq!(o.code, 0, "{}", o.text());
    assert!(o.out.contains("up to date"), "{}", o.text());
}

#[test]
fn a_tarball_install_is_leons_own_to_update() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "{}", o.text());
    assert!(!o.text().contains("brew") && !o.text().contains("dpkg"), "no package manager is named: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
}
