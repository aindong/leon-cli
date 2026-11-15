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

/// The manifest's values end up inside Ruby strings that Homebrew runs on every user's machine, so
/// a value that could close the string or interpolate is refused, and nothing is written.
#[test]
fn a_manifest_value_that_could_escape_a_ruby_string_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let good = tmp.path().join("good");
    signed_release(&good, tmp.path());
    let manifest = std::fs::read_to_string(good.join("latest.json")).unwrap();
    type Edit = fn(&mut Value);
    let edits: [(&str, Edit); 5] = [
        ("a quote in the version", |m| m["version"] = "0.1.0\"\n  system \"id\"\n  x = \"".into()),
        ("interpolation in a sha256", |m| m["files"][0]["sha256"] = "#{`id`}".into()),
        ("a quote in a url", |m| m["files"][2]["url"] = "https://example.invalid/x\"; system(\"id\"); \"".into()),
        ("interpolation in a url", |m| m["files"][3]["url"] = "https://example.invalid/#{`id`}".into()),
        ("a url that is not https", |m| m["files"][4]["url"] = "http://example.invalid/leon.tar.gz".into()),
    ];
    for (i, (what, edit)) in edits.iter().enumerate() {
        let (dir, tap) = (tmp.path().join(format!("m{i}")), tmp.path().join(format!("tap{i}")));
        let mut m: Value = serde_json::from_str(&manifest).unwrap();
        edit(&mut m);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir(&tap).unwrap();
        std::fs::write(dir.join("latest.json"), m.to_string()).unwrap();
        let o = script("release-homebrew.sh", &[dir.to_str().unwrap(), tap.to_str().unwrap()], &[]);
        assert!(!o.status.success(), "{what}: release-homebrew.sh accepted it: {}", text(&o));
        assert_eq!(std::fs::read_dir(&tap).unwrap().count(), 0, "{what}: nothing is written");
    }
    let (dir, tap) = (good.to_str().unwrap(), tmp.path().join("tap-base"));
    std::fs::create_dir(&tap).unwrap();
    let bases = [
        "https://example.invalid/\"; system(\"id\"); \"",
        "https://example.invalid/#{`id`}",
        "ftp://example.invalid/x",
        "file:///tmp/x\n\"; system(\"id\"); \"", // a good first line does not excuse the second (found in review)
    ];
    for base in bases {
        let o = script("release-homebrew.sh", &["--url-base", base, dir, tap.to_str().unwrap()], &[]);
        assert!(!o.status.success(), "--url-base {base} was accepted: {}", text(&o));
        assert_eq!(std::fs::read_dir(&tap).unwrap().count(), 0, "--url-base {base}: nothing is written");
    }
    let o =
        script("release-homebrew.sh", &["--url-base", "file:///tmp/some_dir/x-1.2", dir, tap.to_str().unwrap()], &[]);
    assert!(o.status.success(), "a plain file:// base is fine: {}", text(&o));
}

/// The cask spells one URL pattern for both arches instead of copying the manifest's URLs, so a
/// manifest whose app is somewhere else is refused, not turned into a cask that points elsewhere.
/// (Found in review.)
#[test]
fn an_app_url_the_cask_would_not_reproduce_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let (good, dir, tap) = (tmp.path().join("good"), tmp.path().join("files"), tmp.path().join("tap"));
    signed_release(&good, tmp.path());
    let mut m: Value = serde_json::from_str(&std::fs::read_to_string(good.join("latest.json")).unwrap()).unwrap();
    let app = m["files"].as_array_mut().unwrap().iter_mut().find(|f| f["kind"] == "app").unwrap();
    app["url"] = format!("https://downloads.example.org/{}", app["name"].as_str().unwrap()).into();
    std::fs::create_dir(&dir).unwrap();
    std::fs::create_dir(&tap).unwrap();
    std::fs::write(dir.join("latest.json"), m.to_string()).unwrap();
    let o = script("release-homebrew.sh", &[dir.to_str().unwrap(), tap.to_str().unwrap()], &[]);
    assert!(!o.status.success(), "the cask ignores the manifest's app URL: {}", text(&o));
    assert_eq!(std::fs::read_dir(&tap).unwrap().count(), 0, "nothing is written");
}
