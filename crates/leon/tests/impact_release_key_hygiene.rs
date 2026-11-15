//! Impact check for the release pipeline: `scripts/release-manifest.sh` is the one place the real
//! signing key is handled in CI. It may put the key in a private temp file to sign with, and
//! nothing of it may be left afterwards or appear in what the script prints.

mod release_kit;

use std::os::unix::fs::PermissionsExt;

use release_kit::{fake_release, key, script, text};

#[test]
fn signing_leaves_no_key_behind_and_prints_none() {
    let tmp = tempfile::tempdir().unwrap();
    let (dir, scratch) = (tmp.path().join("files"), tmp.path().join("scratch"));
    fake_release(&dir);
    std::fs::create_dir(&scratch).unwrap();
    let key = key(tmp.path(), "k");
    let env = [("LEON_RELEASE_KEY", key.secret.as_str()), ("TMPDIR", scratch.to_str().unwrap())];

    let o = script("release-manifest.sh", &["--complete", dir.to_str().unwrap()], &env);
    assert!(o.status.success(), "{}", text(&o));
    let secret_line = key.secret.lines().last().unwrap();
    assert!(secret_line.len() > 100 && !text(&o).contains(secret_line), "the secret key is in the output");
    let left: Vec<_> = std::fs::read_dir(&scratch).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(left.is_empty(), "the key's temp dir is gone after signing: {left:?}");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        let body = std::fs::read(&p).unwrap();
        let found = body.windows(secret_line.len()).any(|w| w == secret_line.as_bytes());
        assert!(!found, "the secret key is in {}", p.display());
    }

    // a failing run cleans up too: a stray file stops it after the key was accepted
    std::fs::write(dir.join("stray.bin"), "x").unwrap();
    let o = script("release-manifest.sh", &["--complete", dir.to_str().unwrap()], &env);
    assert!(!o.status.success());
    assert_eq!(std::fs::read_dir(&scratch).unwrap().count(), 0, "a failed run leaves nothing in TMPDIR");
    assert!(!text(&o).contains(secret_line));
}

#[test]
fn the_key_file_is_private_while_signing() {
    // minisign is replaced by a script that records the mode of the key file it is handed
    let tmp = tempfile::tempdir().unwrap();
    let (dir, bin, scratch) = (tmp.path().join("files"), tmp.path().join("bin"), tmp.path().join("scratch"));
    fake_release(&dir);
    for d in [&bin, &scratch] {
        std::fs::create_dir(d).unwrap();
    }
    let real = String::from_utf8(
        std::process::Command::new("sh").args(["-c", "command -v minisign"]).output().unwrap().stdout,
    )
    .unwrap();
    let spy = format!(
        "#!/bin/sh\nprev=\nfor a in \"$@\"; do\n  if [ \"$prev\" = -s ]; then ls -ld \"$a\" \"$(dirname \"$a\")\" >> \"{}\"; fi\n  prev=$a\ndone\nexec {} \"$@\"\n",
        tmp.path().join("modes.txt").display(),
        real.trim()
    );
    std::fs::write(bin.join("minisign"), spy).unwrap();
    std::fs::set_permissions(bin.join("minisign"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let key = key(tmp.path(), "k");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let env =
        [("LEON_RELEASE_KEY", key.secret.as_str()), ("TMPDIR", scratch.to_str().unwrap()), ("PATH", path.as_str())];
    let o = script("release-manifest.sh", &["--complete", dir.to_str().unwrap()], &env);
    assert!(o.status.success(), "{}", text(&o));

    let modes = std::fs::read_to_string(tmp.path().join("modes.txt")).expect("minisign was handed a key file");
    assert_eq!(modes.lines().count(), 20, "one key file and its dir per signature: {modes}");
    for line in modes.lines() {
        let mode = line.split_whitespace().next().unwrap().trim_end_matches(['@', '+', '.']);
        assert!(mode == "-rw-------" || mode == "drwx------", "only the owner can read the key: {line}");
    }
}
