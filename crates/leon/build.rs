//! Sets `LEON_VERSION` for `leon --version`: the package version and the commit it was built from,
//! `0.1.0 (1a2b3c4d5e6f)`. `LEON_GIT_HASH` overrides git (a build with no `.git`, such as Docker
//! with a worktree's source); with neither, it is the bare version.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let o = Command::new("git").args(args).output().ok()?;
    let s = String::from_utf8(o.stdout).ok()?.trim().to_string();
    (o.status.success() && !s.is_empty()).then_some(s)
}

fn main() {
    println!("cargo:rerun-if-env-changed=LEON_GIT_HASH");
    // a new commit or checkout moves HEAD or the branch it names (loose or packed)
    let mut watch = vec!["HEAD".to_string(), "packed-refs".to_string()];
    watch.extend(git(&["rev-parse", "--symbolic-full-name", "HEAD"]));
    for p in watch {
        if let Some(path) = git(&["rev-parse", "--git-path", &p]) {
            if std::path::Path::new(&path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    let hash = std::env::var("LEON_GIT_HASH")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| git(&["rev-parse", "--short=12", "HEAD"]));
    let version = env!("CARGO_PKG_VERSION");
    match hash {
        Some(h) => println!("cargo:rustc-env=LEON_VERSION={version} ({h})"),
        None => println!("cargo:rustc-env=LEON_VERSION={version}"),
    }
}
