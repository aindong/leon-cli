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
    let other = format!("leon-{VERSION}-x86_64-linux.tar.gz");
    type Change<'a> = Box<dyn Fn(&Path) + 'a>;
    // (what was done, the name the error has to mention, how)
    let changes: Vec<(&str, &str, Change)> = vec![
        ("a byte of the app zip", &zip, Box::new(|d| flip(&d.join(&zip)))),
        ("a byte of a .deb", &deb, Box::new(|d| flip(&d.join(&deb)))),
        ("a byte of the manifest", "latest.json", Box::new(|d| flip(&d.join("latest.json")))),
        ("a byte of SHA256SUMS", "SHA256SUMS", Box::new(|d| flip(&d.join("SHA256SUMS")))),
        (
            "another file's signature",
            &tarball,
            Box::new(|d| {
                std::fs::copy(d.join(format!("{other}.minisig")), d.join(format!("{tarball}.minisig"))).unwrap();
            }),
        ),
        (
            "a signature removed",
            &tarball,
            Box::new(|d| std::fs::remove_file(d.join(format!("{tarball}.minisig"))).unwrap()),
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

/// "Any other file in DIR is an error", whatever it is called: a name grep would read as an option,
/// a name with a newline in it, a signature of nothing, a hidden file. (Found in review.)
#[test]
fn a_file_the_manifest_does_not_list_fails_whatever_its_name() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good");
    let key = signed_release(&good, tmp.path());
    let zip = format!("Leon-{VERSION}-macos-arm64.zip");
    let names = [format!("-e{zip}"), format!("evil\n{zip}"), "payload.minisig".to_string(), ".hidden".to_string()];
    for (i, name) in names.iter().enumerate() {
        let dir = tmp.path().join(format!("case{i}"));
        copy_dir(&good, &dir);
        std::fs::write(dir.join(name), "unsigned").unwrap();
        let o = verify(&dir, &key.keys_file, &["--complete"]);
        assert!(!o.status.success(), "an unsigned extra file passed: {name:?}: {}", text(&o));
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

    // a hand-edited keys file whose last line has no newline still pins that key (found in review)
    let unterminated = tmp.path().join("unterminated.pub");
    std::fs::write(&unterminated, format!("# a key\n{}", signer.public)).unwrap();
    let o = verify(&dir, &unterminated, &["--complete"]);
    assert!(o.status.success(), "the last line is a key too: {}", text(&o));

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

/// A manifest with two entries for one platform leaves a reader guessing which to take, so the
/// verifier refuses it even when the key signed it. (docs/releases.md tells an updater the same.)
#[test]
fn two_entries_for_one_platform_are_refused_even_when_signed() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("files");
    let key = signed_release(&dir, tmp.path());
    let sec = tmp.path().join("resign.sec");
    std::fs::write(&sec, &key.secret).unwrap();
    let sign = |name: &str| {
        let _ = std::fs::remove_file(dir.join(format!("{name}.minisig")));
        let o =
            release_kit::minisign(&["-S", "-W", "-s", sec.to_str().unwrap(), "-m", dir.join(name).to_str().unwrap()]);
        assert!(o.status.success(), "{}", text(&o));
    };
    // a ninth file, signed and listed like the others, for a platform that already has its app
    let extra = "Leon-extra-macos-arm64.zip";
    std::fs::write(dir.join(extra), "another app for the same Macs").unwrap();
    sign(extra);
    let sum =
        release_kit::sh(&dir, &format!("(sha256sum {extra} 2>/dev/null || shasum -a 256 {extra}) | cut -d' ' -f1"));
    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
    std::fs::write(dir.join("SHA256SUMS"), format!("{sums}{}  {extra}\n", sum.trim())).unwrap();
    let mut m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();
    let mut entry =
        m["files"].as_array().unwrap().iter().find(|f| f["kind"] == "app" && f["arch"] == "arm64").unwrap().clone();
    entry["name"] = extra.into();
    entry["url"] = format!("https://github.com/aindong/leon-cli/releases/download/v{VERSION}/{extra}").into();
    entry["size"] = std::fs::metadata(dir.join(extra)).unwrap().len().into();
    entry["sha256"] = sum.trim().into();
    entry["signature"] = std::fs::read_to_string(dir.join(format!("{extra}.minisig"))).unwrap().into();
    m["files"].as_array_mut().unwrap().push(entry);
    std::fs::write(dir.join("latest.json"), m.to_string()).unwrap();
    sign("SHA256SUMS");
    sign("latest.json");

    let o = verify(&dir, &key.keys_file, &["--complete"]);
    assert!(!o.status.success(), "two apps for macOS arm64 were accepted: {}", text(&o));
}
