//! Update criterion 3 (.agent/plans/2026-11-16-self-update-core.md): only a release that is newer
//! by semver order is taken, with the parts compared as numbers, and the tag must be `v` plus the
//! version. An old release is still validly signed, so this is all that stops a downgrade.

mod release_kit;
mod update_kit;

use serde_json::json;
use update_kit::{assert_refused, run, Install, Out, Release, Server, VERSION};

/// `leon self-update ARGS` as a leon that is version `running`.
fn update_as(leon: &Install, rel: &Release, srv: &Server, running: &str, args: &[&str]) -> Out {
    let seam = json!({ "keys": rel.key.keys_file, "version": running });
    run(leon.cmd_with(&srv.url(), &seam).arg("self-update").args(args), 60)
}

/// A release whose manifest says it is `version`.
fn release(version: &str) -> Release {
    let rel = Release::new();
    rel.edit(&format!(".version = \"{version}\" | .tag = \"v{version}\""));
    rel
}

fn assert_not_taken(leon: &Install, rel: &Release, srv: &Server, running: &str) {
    let before = leon.state();
    for args in [&[][..], &["--check"]] {
        let o = update_as(leon, rel, srv, running, args);
        assert_eq!(o.code, 0, "{running} is not older than the release: {}", o.text());
        assert!(o.out.contains("up to date"), "it says up to date: {}", o.text());
    }
    leon.assert_unchanged(before, "the release is not newer");
    assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no file was fetched: {:?}", srv.asked());
}

#[test]
fn the_same_version_is_not_taken() {
    let rel = Release::new();
    assert_not_taken(&Install::cli(), &rel, &rel.serve(), VERSION);
}

#[test]
fn an_older_release_is_not_taken() {
    let rel = Release::new();
    assert_not_taken(&Install::cli(), &rel, &rel.serve(), "99.0.0");
}

#[test]
fn a_pre_release_that_sorts_lower_is_not_taken() {
    let rel = release("99.0.0-rc.1");
    let (srv, leon) = (rel.serve(), Install::cli());
    assert_not_taken(&leon, &rel, &srv, "99.0.0");
    // and the other way round it is newer: the release after its own pre-release
    let rel = release("99.0.0");
    let o = update_as(&leon, &rel, &rel.serve(), "99.0.0-rc.1", &["--check"]);
    assert_eq!(o.code, 10, "99.0.0 is newer than 99.0.0-rc.1: {}", o.text());
}

#[test]
fn a_released_leon_is_never_offered_a_pre_release() {
    // newer by semver order, and correctly signed: still only for a leon that is a pre-release
    let rel = release("99.0.0-rc.1");
    let (srv, leon) = (rel.serve(), Install::cli());
    assert_not_taken(&leon, &rel, &srv, "0.1.0");
    let o = update_as(&leon, &rel, &srv, "99.0.0-rc.0", &["--check"]);
    assert_eq!(o.code, 10, "a pre-release leon follows the pre-releases: {}", o.text());
}

#[test]
fn parts_are_compared_as_numbers() {
    let rel = release("0.10.0");
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = update_as(&leon, &rel, &srv, "0.9.0", &["--check"]);
    assert_eq!(o.code, 10, "0.10.0 is newer than 0.9.0: {}", o.text());
    let rel = release("0.9.0");
    assert_not_taken(&leon, &rel, &rel.serve(), "0.10.0");
}

#[test]
fn a_tag_that_is_not_v_plus_the_version_is_refused() {
    let rel = Release::new();
    rel.edit(".tag = \"v9.9.9\"");
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    assert_refused(&leon.update(&rel, &srv, &[]), &["tag", "v9.9.9"]);
    leon.assert_unchanged(before, "a tag that does not match the version");
}

#[test]
fn a_version_that_is_not_one_is_refused() {
    for odd in ["banana", "1.0", "01.2.3", "99"] {
        let rel = release(odd);
        let (srv, leon) = (rel.serve(), Install::cli());
        let before = leon.state();
        assert_refused(&leon.update(&rel, &srv, &[]), &["version", odd]);
        leon.assert_unchanged(before, "a version that is not semver");
    }
}
