//! Release pipeline criterion 3: `scripts/release-manifest.sh` signs a full release with a
//! throwaway key and writes `latest.json`; `scripts/release-verify.sh` accepts it; and the manifest
//! has exactly the fields docs/releases.md documents, since the updater is written against that table.

mod release_kit;

use std::collections::BTreeSet;

use release_kit::{full_set, repo, signed_release, text, verify, COMMIT, VERSION};
use serde_json::Value;

fn kind(v: &Value) -> &'static str {
    match v {
        Value::String(_) => "string",
        Value::Number(_) => "number",
        Value::Object(_) => "object",
        Value::Array(_) => "array",
        Value::Bool(_) => "boolean",
        Value::Null => "null",
    }
}

/// Every field as `path type`: `files[].sha256 string`, `min_os.macos string`.
fn fields(v: &Value, path: &str, out: &mut BTreeSet<String>) {
    match v {
        Value::Object(map) => {
            for (k, v) in map {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                out.insert(format!("{p} {}", kind(v)));
                fields(v, &p, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| fields(i, &format!("{path}[]"), out)),
        _ => {}
    }
}

/// The rows of the manifest table in docs/releases.md: "| `files[].sha256` | string | … |".
fn documented() -> BTreeSet<String> {
    let doc = std::fs::read_to_string(repo().join("docs/releases.md")).expect("docs/releases.md exists");
    let rows = doc.lines().filter_map(|l| {
        let mut cells = l.strip_prefix("| `")?.split('|').map(str::trim);
        Some(format!("{} {}", cells.next()?.trim_end_matches('`'), cells.next()?))
    });
    rows.collect()
}

#[test]
fn a_full_release_is_signed_verifies_and_matches_the_documented_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("files");
    let key = signed_release(&dir, tmp.path());

    let mut signed: Vec<String> = full_set().into_iter().map(|f| f.0).collect();
    signed.extend(["SHA256SUMS".to_string(), "latest.json".to_string()]);
    for name in &signed {
        assert!(dir.join(name).is_file(), "{name} is in the release");
        let sig = dir.join(format!("{name}.minisig"));
        let o = release_kit::minisign(&["-V", "-P", &key.public, "-m", dir.join(name).to_str().unwrap()]);
        assert!(sig.is_file() && o.status.success(), "{name} has a signature by the key: {}", text(&o));
    }
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), signed.len() * 2, "nothing else is written");

    let o = verify(&dir, &key.keys_file, &["--complete"]);
    assert!(o.status.success(), "release-verify.sh rejects a good release: {}", text(&o));

    let m: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();
    let mut got = BTreeSet::new();
    fields(&m, "", &mut got);
    assert_eq!(got, documented(), "latest.json's fields (left) are the ones docs/releases.md documents (right)");

    let base = "https://github.com/aindong/leon-cli/releases";
    assert_eq!(m["schema"], 1);
    assert_eq!(m["version"], VERSION);
    assert_eq!(m["tag"], format!("v{VERSION}"));
    assert_eq!(m["commit"], COMMIT);
    assert_eq!(m["notes_url"], format!("{base}/tag/v{VERSION}"));
    assert_eq!(m["min_os"]["macos"], "11.0", "LSMinimumSystemVersion in scripts/bundle.sh");
    assert_eq!(m["min_os"]["linux_glibc"], "2.34", "the .deb's libc6 floor");
    let date = m["date"].as_str().unwrap().as_bytes();
    let shape = b"0000-00-00T00:00:00Z";
    assert_eq!(date.len(), shape.len(), "date is UTC, to the second");
    for (d, s) in date.iter().zip(shape) {
        assert!(if *s == b'0' { d.is_ascii_digit() } else { d == s }, "date is YYYY-MM-DDTHH:MM:SSZ: {}", m["date"]);
    }

    let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
    let files = m["files"].as_array().unwrap();
    assert_eq!(files.len(), 8);
    for (name, kind, os, arch) in full_set() {
        let f = files.iter().find(|f| f["name"] == name.as_str()).unwrap_or_else(|| panic!("{name} is listed"));
        assert_eq!((f["kind"].as_str(), f["os"].as_str(), f["arch"].as_str()), (Some(kind), Some(os), Some(arch)));
        assert_eq!(f["url"], format!("{base}/download/v{VERSION}/{name}"));
        assert_eq!(f["size"], std::fs::metadata(dir.join(&name)).unwrap().len());
        let sha = f["sha256"].as_str().unwrap();
        assert!(
            sha.len() == 64 && sums.contains(&format!("{sha}  {name}\n")),
            "{name}'s sha256 is its SHA256SUMS line"
        );
        let sig = std::fs::read_to_string(dir.join(format!("{name}.minisig"))).unwrap();
        assert_eq!(f["signature"], sig.as_str(), "{name}'s signature is its .minisig file's text");
    }
    assert_eq!(sums.lines().count(), 8, "SHA256SUMS lists the release files, not itself or the manifest");
}

#[test]
fn signing_needs_the_key_and_refuses_a_file_it_does_not_know() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("files");
    release_kit::fake_release(&dir);
    let o = release_kit::script("release-manifest.sh", &["--complete", dir.to_str().unwrap()], &[]);
    assert!(!o.status.success() && text(&o).contains("LEON_RELEASE_KEY"), "no key, no release: {}", text(&o));
    assert!(!dir.join("latest.json").exists(), "nothing is written without the key");

    let key = release_kit::key(tmp.path(), "k");
    for stray in ["notes.txt", "leon-99.0.0-x86_64-linux.tar.gz"] {
        std::fs::write(dir.join(stray), "x").unwrap();
        let o = release_kit::manifest(&dir, &key, &["--complete"]);
        assert!(!o.status.success() && text(&o).contains(stray), "{stray} is not a release file: {}", text(&o));
        std::fs::remove_file(dir.join(stray)).unwrap();
    }
    assert!(release_kit::manifest(&dir, &key, &["--complete"]).status.success());
}

/// The table's rows for `kind`, `os` and `arch` name every value the manifest can hold, in
/// backticks, and the `arch` row says the two spellings file names use that the field never does.
/// A new value, or wording that sends a reader to the file name, fails here.
#[test]
fn the_documented_values_are_the_ones_the_manifest_holds() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("files");
    signed_release(&dir, tmp.path());
    let m: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();
    let doc = std::fs::read_to_string(repo().join("docs/releases.md")).unwrap();
    for field in ["kind", "os", "arch"] {
        let row = doc.lines().find(|l| l.starts_with(&format!("| `files[].{field}` |"))).expect("a row for the field");
        let meaning = row.split('|').nth(3).unwrap();
        let held: BTreeSet<&str> = m["files"].as_array().unwrap().iter().map(|f| f[field].as_str().unwrap()).collect();
        let mut named: BTreeSet<&str> = meaning.split('`').skip(1).step_by(2).collect();
        if field == "arch" {
            // the two spellings the row quotes from file names, which the field never holds
            assert!(named.remove("aarch64") && named.remove("amd64"), "the row says file names differ: {row}");
            assert!(!row.contains("whatever"), "the row does not send the reader to the file name: {row}");
        }
        assert_eq!(held, named, "files[].{field}: the manifest's values (left), the row's (right): {row}");
    }
    let names: Vec<&str> = m["files"].as_array().unwrap().iter().map(|f| f["name"].as_str().unwrap()).collect();
    assert!(names.iter().any(|n| n.contains("aarch64")) && names.iter().any(|n| n.contains("amd64")), "{names:?}");
    assert!(m["files"].as_array().unwrap().iter().all(|f| f["arch"] != "aarch64" && f["arch"] != "amd64"));
}

/// The manifest names the commit the files were built from, not whatever checkout signs them:
/// `scripts/release.sh` records it in the directory (BUILD-COMMIT), and a `LEON_GIT_HASH` that
/// says otherwise, or no word on the commit at all, stops the signing.
#[test]
fn files_built_from_another_commit_are_not_signed_as_this_one() {
    let tmp = tempfile::tempdir().unwrap();
    let key = release_kit::key(tmp.path(), "k");
    let built = "aaaaaaaaaaaa"; // the commit release.sh built; the kit's LEON_GIT_HASH is another
    let sign = |dir: &std::path::Path, hash: &str| {
        let env = [("LEON_RELEASE_KEY", key.secret.as_str()), ("LEON_GIT_HASH", hash)];
        release_kit::script("release-manifest.sh", &["--complete", dir.to_str().unwrap()], &env)
    };
    let fresh = |name: &str, record: Option<&str>| {
        let dir = tmp.path().join(name);
        release_kit::fake_release(&dir);
        if let Some(r) = record {
            std::fs::write(dir.join("BUILD-COMMIT"), format!("{r}\n")).unwrap();
        }
        dir
    };

    let dir = fresh("other", Some(built));
    let o = sign(&dir, COMMIT);
    assert!(!o.status.success(), "files built from {built} were signed as {COMMIT}: {}", text(&o));
    assert!(text(&o).contains(built) && text(&o).contains(COMMIT), "the error names both commits: {}", text(&o));
    assert!(!dir.join("latest.json").exists(), "nothing is signed");

    // the record alone is enough, and it is what the manifest says; it is not a release file
    let o = sign(&dir, "");
    assert!(o.status.success(), "{}", text(&o));
    let m: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("latest.json")).unwrap()).unwrap();
    assert_eq!(m["commit"], built);
    assert!(!dir.join("BUILD-COMMIT").exists(), "the record is consumed: the signed manifest carries the commit now");
    assert!(verify(&dir, &key.keys_file, &["--complete"]).status.success());

    // a full hash that agrees is fine (CI passes the run's 40-digit commit)
    let dir = fresh("agree", Some(built));
    assert!(sign(&dir, &built.repeat(4)[..40]).status.success());

    // no record and no LEON_GIT_HASH: it will not guess from the checkout that happens to run it
    let dir = fresh("unknown", None);
    let o = sign(&dir, "");
    assert!(!o.status.success() && text(&o).contains("LEON_GIT_HASH"), "{}", text(&o));
    assert!(!dir.join("latest.json").exists());

    for bad in ["not-a-hash", "abc", ""] {
        let dir = fresh(&format!("bad-{}", bad.len()), Some(bad));
        let o = sign(&dir, "");
        assert!(!o.status.success(), "a record of {bad:?} was accepted: {}", text(&o));
    }
}
