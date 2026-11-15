//! Update criterion 5 (.agent/plans/2026-11-16-self-update-core.md): a downloaded file is taken
//! only if its sha256 is the manifest's and its own signature verifies, and no more of it is read
//! than the manifest's size.

mod release_kit;
mod update_kit;

use update_kit::{assert_refused, fake_leon, name, run, sha256, tarball, Install, Mode, Release};

/// Another CLI tarball: a leon that would say it is 6.6.6.
fn other_tarball(rel: &Release) -> Vec<u8> {
    let leon = rel.tmp.path().join("other-leon");
    std::fs::write(&leon, fake_leon("6.6.6")).unwrap();
    tarball(&name("cli"), &leon)
}

/// A jq filter that sets fields of this machine's CLI entry.
fn cli_entry(set: &str) -> String {
    format!("(.files[] | select(.name == \"{}\")) |= ({set})", name("cli"))
}

#[test]
fn a_signed_file_with_another_sha256_is_refused() {
    // a file the key really signed (an older release's, say) under a manifest that names another:
    // only the sha256 tells them apart
    let rel = Release::new();
    let other = other_tarball(&rel);
    let signature = serde_json::to_string(&rel.minisig_of(&other)).unwrap();
    rel.edit(&cli_entry(&format!(".signature = {signature} | .size = {}", other.len())));
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set(&name("cli"), Mode::Bytes(other));
    let before = leon.state();
    assert_refused(&leon.update(&rel, &srv, &[]), &[&name("cli"), "sha256"]);
    leon.assert_unchanged(before, "a file with another sha256");
}

#[test]
fn a_file_whose_signature_does_not_verify_is_refused() {
    // the manifest names the served file's sha256 and size, but carries the old file's signature
    let rel = Release::new();
    let other = other_tarball(&rel);
    rel.edit(&cli_entry(&format!(".sha256 = \"{}\" | .size = {}", sha256(&other), other.len())));
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set(&name("cli"), Mode::Bytes(other));
    let before = leon.state();
    let o = leon.update(&rel, &srv, &[]);
    assert_refused(&o, &[&name("cli"), "does not match its signature"]);
    assert!(!o.err.contains("sha256"), "the sha256 was right; the signature is what failed: {}", o.err);
    leon.assert_unchanged(before, "a file whose signature does not verify");
}

#[test]
fn a_file_that_never_ends_is_cut_off_at_the_manifests_size() {
    let rel = Release::new();
    let size = rel.read(&name("cli")).len() as u64;
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set(&name("cli"), Mode::Endless);
    let before = leon.state();
    let o = run(leon.cmd(&rel, &srv).arg("self-update"), 20);
    assert_refused(&o, &[&name("cli"), "larger than"]);
    assert!(srv.sent() < size + (16 << 20), "leon hung up near the size ({size}): the server sent {}", srv.sent());
    leon.assert_unchanged(before, "a file that never ends");
}

#[test]
fn a_file_that_ends_early_is_refused() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set(&name("cli"), Mode::Truncated);
    let before = leon.state();
    assert_refused(&leon.update(&rel, &srv, &[]), &[&name("cli"), "ended early"]);
    leon.assert_unchanged(before, "a file that ends early");
}

#[test]
fn a_file_that_is_not_there_is_a_failure_not_no_release_yet() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set(&name("cli"), Mode::Status(404));
    let before = leon.state();
    let o = leon.update(&rel, &srv, &[]);
    assert_refused(&o, &[&name("cli"), "answered 404"]);
    assert!(!o.text().contains("no release yet"), "a release with a missing file is not no release: {}", o.text());
    leon.assert_unchanged(before, "a file that is not there");
}

#[test]
fn a_manifest_or_a_signature_that_never_ends_is_cut_off() {
    for (file, cap) in [("latest.json", 1u64 << 20), ("latest.json.minisig", 4 << 10)] {
        let rel = Release::new();
        let (srv, leon) = (rel.serve(), Install::cli());
        srv.set(file, Mode::Endless);
        let before = leon.state();
        let o = run(leon.cmd(&rel, &srv).arg("self-update"), 20);
        assert_refused(&o, &[file, "larger than"]);
        assert!(srv.sent() < cap + (16 << 20), "leon hung up near its cap ({cap}): the server sent {}", srv.sent());
        leon.assert_unchanged(before, "a manifest that never ends");
    }
}

#[test]
fn the_right_file_is_taken() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert_ne!(leon.state(), before, "the install was replaced");
    assert_eq!(leon.version(), format!("leon {} (fake)", update_kit::VERSION));
    assert!(leon.leftovers().is_empty(), "no staging directory is left: {:?}", leon.leftovers());
}
