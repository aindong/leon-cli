//! Publishing: every local link in the README, changelog, brand/README.md and docs/ points at a
//! file or folder git tracks or would add (not an ignored one). Untracked notes stay on disk in a work tree, so "exists" isn't
//! enough; without git (a source tarball) the check falls back to it.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every path git tracks or would add (not ignored), relative to the repo root; None without git.
fn tracked() -> Option<BTreeSet<PathBuf>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root())
        .args(["ls-files", "-z", "--cached", "--others", "--exclude-standard"])
        .output()
        .ok()?;
    out.status.success().then(|| {
        out.stdout
            .split(|&b| b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| PathBuf::from(String::from_utf8_lossy(p).into_owned()))
            .collect()
    })
}

/// `dir/target` with `.` and `..` folded away, relative to the repo root.
fn resolve(dir: &Path, target: &str) -> PathBuf {
    let mut out = PathBuf::new();
    for part in dir.join(target).components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(p) => out.push(p),
            _ => {}
        }
    }
    out
}

/// The targets of `](target)` and `src="target"`, without their `#anchor`. Deliberately simple: no
/// link in these docs holds a `)`.
fn targets(text: &str) -> Vec<&str> {
    let md = text.split("](").skip(1).map(|s| s.split(')').next().unwrap());
    let html = text.split("src=\"").skip(1).map(|s| s.split('"').next().unwrap());
    md.chain(html).map(|t| t.split('#').next().unwrap()).filter(|t| !t.is_empty() && !t.contains("://")).collect()
}

#[test]
fn kept_docs_link_only_to_tracked_files() {
    let tracked = tracked();
    let published = |p: &Path| match &tracked {
        Some(t) => t.contains(p) || t.iter().any(|f| f.starts_with(p)),
        None => root().join(p).exists(),
    };
    let docs: Vec<PathBuf> = match &tracked {
        Some(t) => t.iter().filter(|p| p.extension().is_some_and(|e| e == "md")).cloned().collect(),
        None => vec!["README.md".into(), "CHANGELOG.md".into(), "brand/README.md".into()],
    };
    let docs: Vec<PathBuf> = docs
        .into_iter()
        .filter(|p| {
            p.starts_with("docs") || ["README.md", "CHANGELOG.md", "brand/README.md"].contains(&p.to_str().unwrap())
        })
        .collect();
    assert!(docs.len() > 15 || tracked.is_none(), "found the docs: {docs:?}");
    for doc in docs {
        let text = std::fs::read_to_string(root().join(&doc)).unwrap();
        for target in targets(&text) {
            let path = resolve(doc.parent().unwrap(), target);
            assert!(published(&path), "{}: links to {target}, which git doesn't track", doc.display());
        }
    }
}
