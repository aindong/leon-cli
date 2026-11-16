//! Update criterion 2 (.agent/plans/2026-11-16-self-update-core.md): the manifest is checked
//! against the pinned keys before anything is read from it, and a refusal says which check failed.

mod release_kit;
mod update_kit;

use serde_json::json;
use update_kit::{assert_refused, run, Install, Mode, Release, OLD};

#[test]
fn a_manifest_changed_after_signing_is_refused_unread() {
    let rel = Release::new();
    let text = String::from_utf8(rel.read("latest.json")).unwrap();
    let changed = text.replace(&format!("\"version\": \"{}\"", update_kit::VERSION), "\"version\": \"9.9.9\"");
    assert_ne!(text, changed, "the fixture changes the manifest");
    rel.put("latest.json", changed.as_bytes());
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    for args in [&[][..], &["--check"]] {
        let o = leon.update(&rel, &srv, args);
        assert_refused(&o, &["latest.json", "does not match its signature"]);
        assert!(!o.text().contains("9.9.9"), "nothing from an unverified manifest is shown: {}", o.text());
    }
    leon.assert_unchanged(before, "a manifest changed after signing");
    assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no file was fetched: {:?}", srv.asked());
}

#[test]
fn a_manifest_without_a_signature_is_refused() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    srv.set("latest.json.minisig", Mode::Status(404));
    assert_refused(&leon.update(&rel, &srv, &[]), &["latest.json", "has no signature"]);
    srv.set("latest.json.minisig", Mode::Bytes(b"this is not a minisign signature\n".to_vec()));
    assert_refused(&leon.update(&rel, &srv, &[]), &["latest.json", "not a minisign signature"]);
    leon.assert_unchanged(before, "an unsigned manifest");
}

#[test]
fn a_manifest_signed_by_a_key_that_is_not_pinned_is_refused() {
    let rel = Release::new();
    let other = release_kit::key(rel.tmp.path(), "someone-else");
    rel.sign_manifest(&other);
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    assert_refused(&leon.update(&rel, &srv, &[]), &["latest.json", "a key this leon does not trust"]);
    leon.assert_unchanged(before, "a manifest signed by an unknown key");
}

#[test]
fn a_leon_with_no_pinned_key_refuses_every_manifest() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let empty = leon.home.join("no-keys.pub");
    std::fs::write(&empty, "# a build from before the first key\n\n").unwrap();
    let seam = json!({ "keys": empty, "version": OLD });
    let o = run(leon.cmd_with(&srv.url(), &seam).arg("self-update"), 60);
    assert_refused(&o, &["no release key", update_kit::RELEASES]);
    leon.assert_unchanged(before, "no pinned key");
}

#[test]
fn a_well_signed_manifest_is_accepted() {
    // the four refusals above mean something only if the same setup, left alone, is accepted
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = leon.update(&rel, &srv, &["--check"]);
    assert_eq!(o.code, 10, "a newer, well signed release is reported: {}", o.text());
}
