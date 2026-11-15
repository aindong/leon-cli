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
