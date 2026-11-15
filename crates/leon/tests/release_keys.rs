//! Release pipeline criterion 5: `scripts/release-keys.sh` makes the signing key, hands the secret
//! half to `gh secret set` and nowhere else, and pins the public half. `gh` here is a fake that
//! keeps what it is given.

mod release_kit;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Output;

use release_kit::{minisign, script, text};

const FAKE_GH: &str = r#"#!/bin/sh
# a fake gh: `secret list` prints the names set so far, `secret set NAME` keeps stdin
case "$1 $2" in
  "secret list") cat "$FAKE_GH/names" 2>/dev/null; exit 0 ;;
  "secret set") cat > "$FAKE_GH/$3" && echo "$3" >> "$FAKE_GH/names" ;;
  *) echo "fake gh: unexpected: $*" >&2; exit 1 ;;
esac
"#;

struct Kit {
    tmp: tempfile::TempDir,
}

impl Kit {
    fn new() -> Kit {
        let tmp = tempfile::tempdir().unwrap();
        for d in ["bin", "gh", "tmp"] {
            std::fs::create_dir(tmp.path().join(d)).unwrap();
        }
        let gh = tmp.path().join("bin/gh");
        std::fs::write(&gh, FAKE_GH).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        Kit { tmp }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    /// `release-keys.sh --keys <this kit's pinned.pub> ARGS`.
    fn keys(&self, args: &[&str]) -> Output {
        self.keys_at(&self.path("pinned.pub"), args)
    }

    fn keys_at(&self, keys_file: &Path, args: &[&str]) -> Output {
        let path = format!("{}:{}", self.path("bin").display(), std::env::var("PATH").unwrap());
        let mut all = vec!["--keys", keys_file.to_str().unwrap()];
        all.extend(args);
        let (gh, tmp) = (self.path("gh"), self.path("tmp"));
        script(
            "release-keys.sh",
            &all,
            &[("PATH", &path), ("FAKE_GH", gh.to_str().unwrap()), ("TMPDIR", tmp.to_str().unwrap())],
        )
    }

    fn pinned(&self) -> Vec<String> {
        let text = std::fs::read_to_string(self.path("pinned.pub")).unwrap_or_default();
        text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(str::to_string).collect()
    }
}

/// Sign a file with the secret key text and verify it with the public key.
fn signs_and_verifies(dir: &Path, secret: &str, public: &str) -> bool {
    let (sec, file) = (dir.join("check.sec"), dir.join("check.bin"));
    std::fs::write(&sec, secret).unwrap();
    std::fs::write(&file, "payload").unwrap();
    let signed = minisign(&["-S", "-W", "-s", sec.to_str().unwrap(), "-m", file.to_str().unwrap()]);
    signed.status.success() && minisign(&["-V", "-P", public, "-m", file.to_str().unwrap()]).status.success()
}

#[test]
fn the_secret_goes_to_gh_only_and_the_public_key_is_pinned() {
    let kit = Kit::new();
    std::fs::write(kit.path("pinned.pub"), "# the keys leon trusts\n").unwrap();
    let o = kit.keys(&[]);
    assert!(o.status.success(), "{}", text(&o));

    let secret = std::fs::read_to_string(kit.path("gh/LEON_RELEASE_KEY")).expect("gh secret set LEON_RELEASE_KEY ran");
    let pinned = kit.pinned();
    assert_eq!(pinned.len(), 1, "one key pinned: {pinned:?}");
    assert!(signs_and_verifies(kit.tmp.path(), &secret, &pinned[0]), "the pinned key verifies what the secret signs");
    let out = text(&o);
    assert!(out.contains(&pinned[0]), "the public key is printed: {out}");
    for line in secret.lines().filter(|l| !l.starts_with("untrusted comment:") && !l.is_empty()) {
        assert!(!out.contains(line), "the secret key is in the output");
    }
    let left: Vec<_> = std::fs::read_dir(kit.path("tmp")).unwrap().map(|e| e.unwrap().path()).collect();
    assert!(left.is_empty(), "the key files are gone from TMPDIR: {left:?}");
    let head = std::fs::read_to_string(kit.path("pinned.pub")).unwrap();
    assert!(head.starts_with("# the keys leon trusts\n"), "the file's own lines are kept: {head}");
    assert!(head.contains("\n# LEON_RELEASE_KEY, added 20"), "the key's line says which secret it is: {head}");
}

/// The secret is not set unless its public key can be pinned: a secret whose public half exists
/// nowhere is a key nobody can check, and the script would then refuse to replace it. (Found in review.)
#[test]
fn a_key_that_cannot_be_pinned_is_not_stored_as_the_secret() {
    let kit = Kit::new();
    let o = kit.keys_at(&kit.path("no-such-dir/pinned.pub"), &[]);
    assert!(!o.status.success() && text(&o).contains("no-such-dir/pinned.pub"), "{}", text(&o));
    assert!(!kit.path("gh/LEON_RELEASE_KEY").exists(), "the secret was set with no public key pinned: {}", text(&o));
    assert_eq!(std::fs::read_dir(kit.path("tmp")).unwrap().count(), 0, "and no key was made");
}

#[test]
fn an_existing_secret_is_never_replaced_and_a_second_name_rotates() {
    let kit = Kit::new();
    assert!(kit.keys(&[]).status.success());
    let (secret, pinned) = (std::fs::read(kit.path("gh/LEON_RELEASE_KEY")).unwrap(), kit.pinned());

    let o = kit.keys(&[]);
    assert!(!o.status.success(), "a second run replaced the secret: {}", text(&o));
    assert!(
        text(&o).contains("LEON_RELEASE_KEY") && text(&o).contains("--name"),
        "says why and what to do: {}",
        text(&o)
    );
    assert_eq!(std::fs::read(kit.path("gh/LEON_RELEASE_KEY")).unwrap(), secret, "the secret is as it was");
    assert_eq!(kit.pinned(), pinned, "nothing new is pinned");

    // a rotation: a new key under a second name, pinned beside the first
    let o = kit.keys(&["--name", "LEON_RELEASE_KEY_2"]);
    assert!(o.status.success(), "{}", text(&o));
    let (second, both) = (std::fs::read_to_string(kit.path("gh/LEON_RELEASE_KEY_2")).unwrap(), kit.pinned());
    assert_eq!(both.len(), 2);
    assert_eq!(both[0], pinned[0], "the first key stays pinned");
    assert!(signs_and_verifies(kit.tmp.path(), &second, &both[1]));
    assert!(!kit.keys(&["--name", "not a secret name"]).status.success(), "a bad secret name is refused");
}
