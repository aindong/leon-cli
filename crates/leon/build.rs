//! Sets `LEON_VERSION` for `leon --version`: the package version and the commit it was built from,
//! `0.1.0 (1a2b3c4d5e6f)`. `LEON_GIT_HASH` overrides git (a build with no `.git`, such as Docker
//! with a worktree's source); with neither, it is the bare version.

use std::path::Path;
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let o = Command::new("git").args(args).output().ok()?;
    let s = String::from_utf8(o.stdout).ok()?.trim().to_string();
    (o.status.success() && !s.is_empty()).then_some(s)
}

fn main() {
    println!("cargo:rerun-if-env-changed=LEON_GIT_HASH");
    let version = env!("CARGO_PKG_VERSION");
    let given =
        std::env::var("LEON_GIT_HASH").ok().filter(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit()));
    // only this workspace's own repo: from a source tarball, git would find a parent dir's repo
    let ours = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.git").exists();
    let hash = given.or_else(|| if ours { git(&["rev-parse", "--short=12", "HEAD"]) } else { None });
    if ours {
        // a commit moves a branch's loose ref (made anew when the branch was packed: cargo scans
        // the dir) and logs/HEAD; a checkout moves HEAD
        for p in ["HEAD", "logs/HEAD", "packed-refs", "refs/heads"] {
            if let Some(path) = git(&["rev-parse", "--git-path", p]).filter(|p| Path::new(p).exists()) {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    match hash {
        Some(h) => println!("cargo:rustc-env=LEON_VERSION={version} ({h})"),
        None => println!("cargo:rustc-env=LEON_VERSION={version}"),
    }
}
