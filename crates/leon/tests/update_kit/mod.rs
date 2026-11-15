//! Shared by the update_* tests: a release signed with a throwaway key (scripts/release-manifest.sh
//! makes it, as for a real one), a loopback server for it that can misbehave, and a leon installed
//! in a temp dir that the tests update. Nothing here touches the network or a real install.
//!
//! The installed leon is a copy of the test binary. It is told which keys to trust and which
//! version it is through `LEON_TEST_UPDATE`, which only a debug build reads.

#![allow(dead_code)]

mod server;

use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::release_kit::{self, Key};
#[allow(unused_imports)] // not every test makes the server misbehave
pub use server::{Mode, Server};

/// The version the fake release has: the checkout's.
pub const VERSION: &str = release_kit::VERSION;
/// The version the installed leon is told it is.
pub const OLD: &str = "0.0.1";
pub const RELEASES: &str = "https://github.com/aindong/leon-cli/releases/latest";

pub fn os() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

pub fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x86_64"
    }
}

/// The release file of `kind` for `os` and this machine's arch.
pub fn name_for(kind: &str, os: &str) -> String {
    let set = release_kit::full_set();
    set.into_iter().find(|(_, k, o, a)| (*k, *o, *a) == (kind, os, arch())).expect("a file of that kind").0
}

/// The release file of `kind` for this machine.
pub fn name(kind: &str) -> String {
    name_for(kind, os())
}

pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A stand-in for the leon binary in a release: it only knows `--version`.
pub fn fake_leon(version: &str) -> Vec<u8> {
    format!("#!/bin/sh\necho \"leon {version} (fake)\"\n").into_bytes()
}

fn write_exe(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A CLI tarball as scripts/release.sh lays one out: one directory named like the tarball, with
/// `leon` and a README in it.
pub fn tarball(name: &str, leon: &Path) -> Vec<u8> {
    let scratch = tempfile::tempdir().unwrap();
    let top = name.trim_end_matches(".tar.gz");
    std::fs::create_dir(scratch.path().join(top)).unwrap();
    std::fs::copy(leon, scratch.path().join(top).join("leon")).unwrap();
    std::fs::write(scratch.path().join(top).join("README.md"), "a fake release\n").unwrap();
    // root as the owner, as in a real release: an unpacker must not try to restore it
    let owner = if cfg!(target_os = "macos") { "--uid 0 --gid 0" } else { "--owner=0 --group=0" };
    release_kit::sh(scratch.path(), &format!("COPYFILE_DISABLE=1 tar -czf out.tar.gz --numeric-owner {owner} '{top}'"));
    std::fs::read(scratch.path().join("out.tar.gz")).unwrap()
}

/// A zip with `app` (a Leon.app) at its top level, as scripts/release.sh makes one.
pub fn app_zip(app: &Path) -> Vec<u8> {
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("out.zip");
    let o = Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(app).arg(&out).output().unwrap();
    assert!(o.status.success(), "ditto: {}", release_kit::text(&o));
    std::fs::read(out).unwrap()
}

/// The least a Leon.app is to the updater: the binary where a bundle keeps it.
pub fn fake_app(dir: &Path, leon: &[u8]) -> PathBuf {
    let app = dir.join("Leon.app");
    write_exe(&app.join("Contents/MacOS/leon"), leon);
    std::fs::write(app.join("Contents/Info.plist"), "<plist version=\"1.0\"><dict/></plist>\n").unwrap();
    app
}

/// The test binary with different bytes, so a release's leon can be told from the installed one:
/// on macOS signed again under another identifier, on Linux with bytes after the ELF image.
pub fn other_build(dir: &Path) -> PathBuf {
    let copy = dir.join("leon-for-the-release");
    std::fs::copy(env!("CARGO_BIN_EXE_leon"), &copy).unwrap();
    if cfg!(target_os = "macos") {
        let sign = ["--force", "--sign", "-", "-i", "dev.leon.update-test"];
        let o = Command::new("codesign").args(sign).arg(&copy).output().unwrap();
        assert!(o.status.success(), "codesign: {}", release_kit::text(&o));
    } else {
        use std::io::Write;
        std::fs::OpenOptions::new().append(true).open(&copy).unwrap().write_all(b"\nthe release's build\n").unwrap();
    }
    copy
}

/// A full fake release in a temp dir, and the throwaway key it is signed with.
pub struct Release {
    pub tmp: tempfile::TempDir,
    pub dir: PathBuf,
    pub key: Key,
}

impl Release {
    /// All eight files, not signed yet. This machine's CLI tarball (and on macOS its app zip)
    /// hold a fake leon that says it is `VERSION`; the other files are filler.
    pub fn unsigned() -> Release {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("release");
        release_kit::fake_release(&dir);
        let keys = tmp.path().join("keys");
        std::fs::create_dir(&keys).unwrap();
        let key = release_kit::key(&keys, "release");
        let rel = Release { tmp, dir, key };
        let leon = rel.tmp.path().join("fake-leon");
        write_exe(&leon, &fake_leon(VERSION));
        rel.put(&name("cli"), &tarball(&name("cli"), &leon));
        if cfg!(target_os = "macos") {
            let app = fake_app(&rel.tmp.path().join("stage"), &fake_leon(VERSION));
            rel.put(&name("app"), &app_zip(&app));
        }
        rel
    }

    /// Sign every file and write the manifest, with scripts/release-manifest.sh.
    pub fn sign(&self) {
        self.sign_args(&["--complete"]);
    }

    /// `sign`, with the script's arguments: none for a release that lacks a file.
    pub fn sign_args(&self, args: &[&str]) {
        let o = release_kit::manifest(&self.dir, &self.key, args);
        assert!(o.status.success(), "release-manifest.sh failed: {}", release_kit::text(&o));
    }

    pub fn new() -> Release {
        let rel = Release::unsigned();
        rel.sign();
        rel
    }

    /// A signed release whose leon for this machine is a real one (`other_build`), for the tests
    /// that go on using the leon they installed. Returns it with that leon's sha256.
    pub fn real() -> (Release, String) {
        let rel = Release::unsigned();
        let leon = other_build(rel.tmp.path());
        rel.put(&name("cli"), &tarball(&name("cli"), &leon));
        if cfg!(target_os = "macos") {
            let app = fake_app(&rel.tmp.path().join("real-stage"), &std::fs::read(&leon).unwrap());
            rel.put(&name("app"), &app_zip(&app));
        }
        rel.sign();
        let sum = sha256(&std::fs::read(&leon).unwrap());
        (rel, sum)
    }

    /// Replace a release file's bytes. Before `sign`: a different release. After it: tampering.
    pub fn put(&self, name: &str, bytes: &[u8]) {
        std::fs::write(self.dir.join(name), bytes).unwrap();
    }

    pub fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.dir.join(name)).unwrap()
    }

    pub fn manifest(&self) -> Value {
        serde_json::from_slice(&self.read("latest.json")).unwrap()
    }

    /// Rewrite latest.json with a jq filter, then sign it again with this release's key: a
    /// manifest its signer really signed, but that scripts/release-manifest.sh would never write.
    pub fn edit(&self, filter: &str) {
        let o = Command::new("jq").arg(filter).arg(self.dir.join("latest.json")).output().unwrap();
        assert!(o.status.success(), "jq {filter}: {}", release_kit::text(&o));
        self.put("latest.json", &o.stdout);
        self.sign_manifest(&self.key);
    }

    /// Sign latest.json as it is now with `key`.
    pub fn sign_manifest(&self, key: &Key) {
        let sec = self.tmp.path().join("resign.sec");
        std::fs::write(&sec, &key.secret).unwrap();
        let _ = std::fs::remove_file(self.dir.join("latest.json.minisig"));
        let o = release_kit::minisign(&[
            "-S",
            "-W",
            "-s",
            sec.to_str().unwrap(),
            "-m",
            self.dir.join("latest.json").to_str().unwrap(),
        ]);
        assert!(o.status.success(), "minisign -S: {}", release_kit::text(&o));
    }

    /// The text of a `.minisig` for `bytes`, made with this release's key.
    pub fn minisig_of(&self, bytes: &[u8]) -> String {
        let (file, sec) = (self.tmp.path().join("to-sign"), self.tmp.path().join("resign.sec"));
        std::fs::write(&file, bytes).unwrap();
        std::fs::write(&sec, &self.key.secret).unwrap();
        let _ = std::fs::remove_file(self.tmp.path().join("to-sign.minisig"));
        let o = release_kit::minisign(&["-S", "-W", "-s", sec.to_str().unwrap(), "-m", file.to_str().unwrap()]);
        assert!(o.status.success(), "minisign -S: {}", release_kit::text(&o));
        std::fs::read_to_string(self.tmp.path().join("to-sign.minisig")).unwrap()
    }

    pub fn serve(&self) -> Server {
        Server::start(&self.dir)
    }
}

/// What a command printed and how it ended.
#[derive(Debug)]
pub struct Out {
    pub code: i32,
    pub out: String,
    pub err: String,
}

impl Out {
    /// stdout then stderr.
    pub fn text(&self) -> String {
        format!("{}{}", self.out, self.err)
    }
}

/// Run `cmd` to its end, or panic after `secs` (macOS has no `timeout`).
pub fn run(cmd: &mut Command, secs: u64) -> Out {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let (mut o, mut e) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = o.read_to_string(&mut s);
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = e.read_to_string(&mut s);
        s
    });
    let end = Instant::now() + Duration::from_secs(secs);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{cmd:?} was still running after {secs} s:\n{}{}", out.join().unwrap(), err.join().unwrap());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Out { code: status.code().unwrap_or(-1), out: out.join().unwrap(), err: err.join().unwrap() }
}

/// Start `cmd` (an update whose seam pauses it at `step`) and return once it has paused there.
/// Panics, killing it, if a minute passes or it ends without pausing.
pub fn spawn_until_paused(cmd: &mut Command, step: &str) -> std::process::Child {
    use std::io::BufRead;
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let stderr = child.stderr.take().unwrap();
    let (tx, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let mut seen = vec![];
    loop {
        match lines.recv_timeout(Duration::from_secs(60)) {
            Ok(line) if line.contains(&format!("paused at {step}")) => return child,
            Ok(line) => seen.push(line),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the update never paused at {step}; it said: {seen:?}");
            }
        }
    }
}

/// Poll until `pred` holds, or panic after `secs`.
pub fn until(what: &str, secs: u64, pred: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while !pred() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A leon in a temp dir: `exe` is what runs, `root` is what an update replaces (the binary, or
/// the app bundle around it).
pub struct Install {
    pub tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub exe: PathBuf,
    pub root: PathBuf,
}

impl Install {
    /// The test binary copied to `<temp>/<rel>`.
    pub fn at(rel: &str) -> Install {
        let tmp = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        // one spelling: macOS temp dirs are behind a symlink, and leon reports real paths
        let home = tmp.path().canonicalize().unwrap();
        let exe = home.join(rel);
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_leon"), &exe).unwrap();
        for private in ["run", "state"] {
            std::fs::create_dir(home.join(private)).unwrap();
            std::fs::set_permissions(home.join(private), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let fake = home.join("fakebin");
        // never the real dpkg or sudo: this dpkg owns nothing, and this sudo leaves a mark
        write_exe(&fake.join("dpkg"), b"#!/bin/sh\nexit 1\n");
        write_exe(&fake.join("sudo"), b"#!/bin/sh\ntouch \"$(dirname \"$0\")/sudo-ran\"\nexit 1\n");
        // the real curl, but every command line it is given is written down first
        let curl = release_kit::sh(&home, "command -v curl");
        // one line each: the arguments hold a newline
        let record =
            "echo \"$*\" | tr '\\n' ' ' >> \"$(dirname \"$0\")/curl-ran\"; echo >> \"$(dirname \"$0\")/curl-ran\"";
        write_exe(&fake.join("curl"), format!("#!/bin/sh\n{record}\nexec {} \"$@\"\n", curl.trim()).as_bytes());
        Install { tmp, home, root: exe.clone(), exe }
    }

    /// A leon someone unpacked from a tarball and put on their PATH.
    pub fn cli() -> Install {
        Install::at("bin/leon")
    }

    /// A leon inside an app bundle, as in `<temp>/Applications/Leon.app`.
    pub fn app() -> Install {
        let mut i = Install::at("Applications/Leon.app/Contents/MacOS/leon");
        i.root = i.home.join("Applications/Leon.app");
        std::fs::write(i.root.join("Contents/Info.plist"), "<plist version=\"1.0\"><dict/></plist>\n").unwrap();
        i
    }

    pub fn fakebin(&self) -> PathBuf {
        self.home.join("fakebin")
    }

    /// The installed leon, with releases at `url` and the debug-only seam set to `seam`.
    pub fn cmd_with(&self, url: &str, seam: &Value) -> Command {
        let mut c = Command::new(&self.exe);
        let path = format!("{}:{}", self.fakebin().display(), std::env::var("PATH").unwrap_or_default());
        c.env("LEON_RELEASE_URL", url)
            .env("LEON_TEST_UPDATE", seam.to_string())
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("LEON_RUNTIME_DIR", self.home.join("run"))
            .env("LEON_STATE_DIR", self.home.join("state"))
            .env("LEON_CONFIG", self.home.join("no-init.lua"))
            .env("LEON_AUTOSTART", "0")
            .env_remove("HOMEBREW_PREFIX")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("LEON_SERVER");
        c
    }

    /// The seam most tests want: trust `rel`'s key, and be version `OLD`.
    pub fn seam(rel: &Release) -> Value {
        json!({ "keys": rel.key.keys_file, "version": OLD })
    }

    /// The installed leon with no `LEON_RELEASE_URL`: the manifest is at the seam's `address`,
    /// and the files are wherever the manifest's `url` fields say.
    pub fn cmd_at_address(&self, rel: &Release, address: &str) -> Command {
        let mut seam = Install::seam(rel);
        seam["address"] = address.into();
        let mut c = self.cmd_with("unused", &seam);
        c.env_remove("LEON_RELEASE_URL");
        c
    }

    pub fn cmd(&self, rel: &Release, srv: &Server) -> Command {
        self.cmd_with(&srv.url(), &Install::seam(rel))
    }

    /// `leon self-update ARGS` against `srv`.
    pub fn update(&self, rel: &Release, srv: &Server, args: &[&str]) -> Out {
        run(self.cmd(rel, srv).arg("self-update").args(args), 60)
    }

    /// What identifies the installed thing: a replaced file or bundle is a new inode.
    pub fn state(&self) -> (u64, u64) {
        let m = std::fs::symlink_metadata(&self.root).unwrap();
        (m.ino(), m.len())
    }

    /// Staging directories an update left beside the install.
    pub fn leftovers(&self) -> Vec<PathBuf> {
        let dir = self.root.parent().unwrap();
        let mut found: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
        found.retain(|p| p.file_name().unwrap().to_string_lossy().starts_with(".leon-update"));
        found
    }

    /// The install is as it was at `before`, with nothing left beside it.
    pub fn assert_unchanged(&self, before: (u64, u64), why: &str) {
        assert_eq!(self.state(), before, "the install was replaced: {why}");
        assert_eq!(self.leftovers(), Vec::<PathBuf>::new(), "a staging directory was left behind: {why}");
    }

    /// The sha256 of the installed binary.
    pub fn sum(&self) -> String {
        sha256(&std::fs::read(&self.exe).unwrap())
    }

    /// What `leon --version` prints now.
    pub fn version(&self) -> String {
        let o = run(Command::new(&self.exe).arg("--version").env("LEON_AUTOSTART", "0"), 30);
        assert_eq!(o.code, 0, "the installed leon runs: {}", o.text());
        o.out.trim().to_string()
    }

    /// Every curl command line leon has run, one per line.
    pub fn curl_ran(&self) -> Vec<String> {
        let log = std::fs::read_to_string(self.fakebin().join("curl-ran")).unwrap_or_default();
        log.lines().map(str::to_string).collect()
    }

    pub fn sudo_ran(&self) -> bool {
        self.fakebin().join("sudo-ran").exists()
    }
}

/// A refusal: exit 1, one line on stderr that holds every one of `needles`, and nothing installed.
pub fn assert_refused(o: &Out, needles: &[&str]) {
    assert_eq!(o.code, 1, "exit 1: {}", o.text());
    assert_eq!(o.err.lines().count(), 1, "one line on stderr:\n{}", o.err);
    for n in needles {
        assert!(o.err.contains(n), "stderr says {n:?}: {}", o.err);
    }
}
