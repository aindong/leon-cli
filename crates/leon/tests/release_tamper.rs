//! Release pipeline criterion 4: `scripts/release-verify.sh` fails on a changed byte in any signed
//! file, on a key that isn't the pinned one, and on a release with a file missing.

mod release_kit;

use std::path::Path;

use release_kit::{copy_dir, full_set, key, manifest, signed_release, text, verify, VERSION};

/// Flip the last byte of `path`.
fn flip(path: &Path) {
    let mut bytes = std::fs::read(path).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_changed_byte_fails_verification() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good");
    let key = signed_release(&good, tmp.path());
    assert!(verify(&good, &key.keys_file, &["--complete"]).status.success());

    let zip = format!("Leon-{VERSION}-macos-arm64.zip");
    let deb = format!("leon_{VERSION}_amd64.deb");
    let tarball = format!("leon-{VERSION}-aarch64-linux.tar.gz");
    type Change = Box<dyn Fn(&Path)>;
    let changes: Vec<(&str, &str, Change)> = vec![
        ("a byte of the app zip", &zip, Box::new(|d| flip(&d.join(format!("Leon-{VERSION}-macos-arm64.zip"))))),
        ("a byte of a .deb", &deb, Box::new(|d| flip(&d.join(format!("leon_{VERSION}_amd64.deb"))))),
        ("a byte of the manifest", "latest.json", Box::new(|d| flip(&d.join("latest.json")))),
        ("a byte of SHA256SUMS", "SHA256SUMS", Box::new(|d| flip(&d.join("SHA256SUMS")))),
        (
            "another file's signature",
            &tarball,
            Box::new(|d| {
                let from = d.join(format!("leon-{VERSION}-x86_64-linux.tar.gz.minisig"));
                std::fs::copy(from, d.join(format!("leon-{VERSION}-aarch64-linux.tar.gz.minisig"))).unwrap();
            }),
        ),
        (
            "a signature removed",
            &tarball,
            Box::new(|d| std::fs::remove_file(d.join(format!("leon-{VERSION}-aarch64-linux.tar.gz.minisig"))).unwrap()),
        ),
        ("a file nobody signed", "extra.bin", Box::new(|d| std::fs::write(d.join("extra.bin"), "x").unwrap())),
    ];
    for (i, (what, names, change)) in changes.iter().enumerate() {
        let dir = tmp.path().join(format!("case{i}"));
        copy_dir(&good, &dir);
        change(&dir);
        let o = verify(&dir, &key.keys_file, &["--complete"]);
        assert!(!o.status.success(), "{what}: release-verify.sh still passed: {}", text(&o));
        assert!(text(&o).contains(names), "{what}: the error names {names}: {}", text(&o));
    }
}

#[test]
fn a_key_that_is_not_pinned_fails_verification() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("files");
    let signer = signed_release(&dir, tmp.path());
    let other = key(tmp.path(), "other");
    let o = verify(&dir, &other.keys_file, &["--complete"]);
    assert!(!o.status.success(), "a release signed by another key passed: {}", text(&o));

    // two pinned keys (a rotation): either may have signed
    let both = tmp.path().join("both.pub");
    std::fs::write(&both, format!("# old\n{}\n\n# new\n{}\n", other.public, signer.public)).unwrap();
    let o = verify(&dir, &both, &["--complete"]);
    assert!(o.status.success(), "the second pinned key signed this: {}", text(&o));

    // no --keys: leon's pinned keys (crates/leon/release-keys.pub), which a test key is never in
    let o = release_kit::script("release-verify.sh", &["--complete", dir.to_str().unwrap()], &[]);
    assert!(!o.status.success(), "a test key passed against leon's pinned keys: {}", text(&o));

    let none = tmp.path().join("none.pub");
    std::fs::write(&none, "# no key yet\n").unwrap();
    let o = verify(&dir, &none, &["--complete"]);
    assert!(!o.status.success() && text(&o).contains("scripts/release-keys.sh"), "says how to pin one: {}", text(&o));
}

#[test]
fn a_release_with_a_file_missing_is_not_complete() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("files");
    release_kit::fake_release(&dir);
    let missing = &full_set()[1].0; // the Intel app
    std::fs::remove_file(dir.join(missing)).unwrap();
    let key = key(tmp.path(), "k");

    let o = manifest(&dir, &key, &["--complete"]);
    assert!(!o.status.success() && text(&o).contains(missing.as_str()), "signing names the gap: {}", text(&o));
    assert!(!dir.join("latest.json").exists(), "an incomplete release gets no manifest");

    // a partial release (a local dry run on one Mac) signs and verifies, but is not --complete
    let o = manifest(&dir, &key, &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(verify(&dir, &key.keys_file, &[]).status.success());
    let o = verify(&dir, &key.keys_file, &["--complete"]);
    assert!(!o.status.success() && text(&o).contains(missing.as_str()), "verify names the gap: {}", text(&o));
}
