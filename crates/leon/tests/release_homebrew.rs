//! Release pipeline criterion 8: `scripts/release-homebrew.sh` writes the tap's cask and formula
//! from a release's `latest.json`: the version, and each file's URL and sha256.

mod release_kit;

use std::process::Command;

use release_kit::{script, signed_release, text, VERSION};
use serde_json::Value;

#[test]
fn the_cask_and_formula_carry_the_manifests_version_urls_and_sums() {
    let tmp = tempfile::tempdir().unwrap();
    let (dir, tap) = (tmp.path().join("files"), tmp.path().join("tap"));
    signed_release(&dir, tmp.path());
    std::fs::create_dir(&tap).unwrap();
    let o = script("release-homebrew.sh", &[dir.to_str().unwrap(), tap.to_str().unwrap()], &[]);
    assert!(o.status.success(), "{}", text(&o));

    let cask = std::fs::read_to_string(tap.join("Casks/leon.rb")).expect("Casks/leon.rb is written");
    let formula = std::fs::read_to_string(tap.join("Formula/leon.rb")).expect("Formula/leon.rb is written");
    let m: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();
    let base = format!("https://github.com/aindong/leon-cli/releases/download/v{VERSION}");
    for f in m["files"].as_array().unwrap() {
        let (name, sha, url) = (f["name"].as_str().unwrap(), f["sha256"].as_str().unwrap(), f["url"].as_str().unwrap());
        match f["kind"].as_str().unwrap() {
            "app" => assert!(cask.contains(sha), "the cask has {name}'s sha256"),
            "cli" => assert!(formula.contains(sha) && formula.contains(url), "the formula has {name}'s URL and sha256"),
            _ => assert!(!cask.contains(sha) && !formula.contains(sha), "{name} is in neither"),
        }
    }
    // the formula's version is the one in its URLs: `brew audit` calls a `version` line redundant
    assert!(cask.contains(&format!("version \"{VERSION}\"")) && !formula.contains("\n  version "), "{formula}");
    // the cask spells its URL once, for both arches, the way Homebrew casks do
    assert!(cask.contains(&format!("{}/Leon-#{{version}}-macos-#{{arch}}.zip", base.replace(VERSION, "#{version}"))));
    assert!(cask.contains("app \"Leon.app\"") && cask.contains("xattr -dr com.apple.quarantine"), "{cask}");
    assert!(!cask.contains("postflight") && !cask.contains("preflight"), "no hook touches the quarantine flag");
    assert!(formula.contains("bin.install \"leon\"") && formula.contains("test do"), "{formula}");

    if cfg!(target_os = "macos") {
        for file in ["Casks/leon.rb", "Formula/leon.rb"] {
            let o = Command::new("ruby").arg("-c").arg(tap.join(file)).output().unwrap();
            assert!(o.status.success(), "{file} is valid Ruby: {}", text(&o));
        }
    }

    // a second run over its own output changes nothing (a re-run of the workflow's tap job)
    let before = (cask, formula);
    assert!(script("release-homebrew.sh", &[dir.to_str().unwrap(), tap.to_str().unwrap()], &[]).status.success());
    let after = (
        std::fs::read_to_string(tap.join("Casks/leon.rb")).unwrap(),
        std::fs::read_to_string(tap.join("Formula/leon.rb")).unwrap(),
    );
    assert_eq!(before, after);
}

#[test]
fn a_release_without_every_homebrew_file_writes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let (dir, tap) = (tmp.path().join("files"), tmp.path().join("tap"));
    release_kit::fake_release(&dir);
    std::fs::remove_file(dir.join(format!("leon-{VERSION}-x86_64-linux.tar.gz"))).unwrap();
    let key = release_kit::key(tmp.path(), "k");
    assert!(release_kit::manifest(&dir, &key, &[]).status.success());
    std::fs::create_dir(&tap).unwrap();
    let o = script("release-homebrew.sh", &[dir.to_str().unwrap(), tap.to_str().unwrap()], &[]);
    assert!(!o.status.success(), "a formula with no Linux x86_64 file was written: {}", text(&o));
    assert!(!tap.join("Formula").exists() && !tap.join("Casks").exists(), "nothing is written");
}
