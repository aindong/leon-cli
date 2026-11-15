//! Shared by the release_* tests: run a release script, make a throwaway minisign key, and fake the
//! eight files of a full release. The tests need `minisign` and `jq` on PATH.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// What the scripts take for the commit (a Docker test run has no usable `.git`).
pub const COMMIT: &str = "0123456789ab";

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run `scripts/<name>`; the caller checks the status.
pub fn script(name: &str, args: &[&str], env: &[(&str, &str)]) -> Output {
    Command::new("bash")
        .arg(repo().join("scripts").join(name))
        .args(args)
        .env("LEON_GIT_HASH", COMMIT)
        .env_remove("LEON_RELEASE_KEY")
        .envs(env.iter().copied())
        .output()
        .unwrap()
}

/// stdout then stderr.
pub fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

/// Run a shell snippet in `dir`; it must succeed.
pub fn sh(dir: &Path, snippet: &str) -> String {
    let o = Command::new("sh").args(["-c", snippet]).current_dir(dir).output().unwrap();
    assert!(o.status.success(), "`{snippet}` failed: {}", text(&o));
    String::from_utf8(o.stdout).unwrap()
}

pub fn minisign(args: &[&str]) -> Output {
    Command::new("minisign")
        .args(args)
        .output()
        .expect("minisign is needed by the release tests: brew install minisign, or apt install minisign")
}

/// A throwaway key pair: the secret key's text (what the `LEON_RELEASE_KEY` secret holds), the
/// public key as one base64 line, and a keys file that pins it.
pub struct Key {
    pub secret: String,
    pub public: String,
    pub keys_file: PathBuf,
}

pub fn key(dir: &Path, name: &str) -> Key {
    let (sec, keys_file) = (dir.join(format!("{name}.sec")), dir.join(format!("{name}.pub")));
    let o = minisign(&["-G", "-W", "-p", keys_file.to_str().unwrap(), "-s", sec.to_str().unwrap()]);
    assert!(o.status.success(), "minisign -G: {}", text(&o));
    let public = std::fs::read_to_string(&keys_file).unwrap().lines().last().unwrap().to_string();
    std::fs::write(&keys_file, format!("# a test key\n{public}\n")).unwrap();
    Key { secret: std::fs::read_to_string(sec).unwrap(), public, keys_file }
}

/// The eight file names of a full release, as (name, kind, os, arch).
pub fn full_set() -> Vec<(String, &'static str, &'static str, &'static str)> {
    let v = VERSION;
    vec![
        (format!("Leon-{v}-macos-arm64.zip"), "app", "macos", "arm64"),
        (format!("Leon-{v}-macos-x86_64.zip"), "app", "macos", "x86_64"),
        (format!("leon-{v}-arm64-macos.tar.gz"), "cli", "macos", "arm64"),
        (format!("leon-{v}-x86_64-macos.tar.gz"), "cli", "macos", "x86_64"),
        (format!("leon-{v}-aarch64-linux.tar.gz"), "cli", "linux", "arm64"),
        (format!("leon-{v}-x86_64-linux.tar.gz"), "cli", "linux", "x86_64"),
        (format!("leon_{v}_arm64.deb"), "deb", "linux", "arm64"),
        (format!("leon_{v}_amd64.deb"), "deb", "linux", "x86_64"),
    ]
}

/// A `.deb` as far as the manifest reads one: an `ar` archive whose `control.tar.xz` holds a
/// control file that depends on glibc 2.34. Written by hand: macOS `ar` only makes Mach-O archives.
fn fake_deb(scratch: &Path, name: &str) -> Vec<u8> {
    std::fs::write(
        scratch.join("control"),
        format!("Package: leon\nDepends: libc6 (>= 2.34), libgcc-s1\nDescription: {name}\n"),
    )
    .unwrap();
    sh(scratch, "tar -cJf control.tar.xz ./control");
    let mut deb = b"!<arch>\n".to_vec();
    for (name, data) in [
        ("debian-binary", b"2.0\n".to_vec()),
        ("control.tar.xz", std::fs::read(scratch.join("control.tar.xz")).unwrap()),
    ] {
        deb.extend(format!("{name:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n", 0, 0, 0, "100644", data.len()).bytes());
        deb.extend(&data);
        if data.len() % 2 == 1 {
            deb.push(b'\n');
        }
    }
    deb
}

/// Fill `dir` with the full set: small files with distinct bytes, and `.deb`s the manifest can read
/// the glibc floor from.
pub fn fake_release(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let scratch = tempfile::tempdir().unwrap();
    for (name, ..) in full_set() {
        if name.ends_with(".deb") {
            std::fs::write(dir.join(&name), fake_deb(scratch.path(), &name)).unwrap();
        } else {
            std::fs::write(dir.join(&name), format!("not really {name}\n").repeat(40)).unwrap();
        }
    }
}

/// `release-manifest.sh` on `dir` with `key`; the caller checks the status.
pub fn manifest(dir: &Path, key: &Key, args: &[&str]) -> Output {
    let mut all = args.to_vec();
    all.push(dir.to_str().unwrap());
    script("release-manifest.sh", &all, &[("LEON_RELEASE_KEY", &key.secret)])
}

/// `release-verify.sh` on `dir` against `keys_file`; the caller checks the status.
pub fn verify(dir: &Path, keys_file: &Path, args: &[&str]) -> Output {
    let mut all = args.to_vec();
    all.extend(["--keys", keys_file.to_str().unwrap(), dir.to_str().unwrap()]);
    script("release-verify.sh", &all, &[])
}

/// A full fake release in `dir`, signed with a new throwaway key.
pub fn signed_release(dir: &Path, keys_dir: &Path) -> Key {
    fake_release(dir);
    let key = key(keys_dir, "release");
    let o = manifest(dir, &key, &["--complete"]);
    assert!(o.status.success(), "release-manifest.sh failed: {}", text(&o));
    key
}

/// Copy every file of `from` into a new dir `to`.
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let p = e.unwrap().path();
        std::fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
    }
}
