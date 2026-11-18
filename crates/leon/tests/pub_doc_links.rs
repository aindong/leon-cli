//! Publishing: every local link in the README, changelog, brand/README.md and docs/ points at a
//! file or folder git tracks or would add (not an ignored one). Untracked notes stay on disk in a work tree, so "exists" isn't
//! enough; without git (a source tarball) the check falls back to it. Translations (docs/i18n/) are
//! scripts/i18n-check.sh's: these tests neither read them nor follow links into them, since the
//! README links each language before its translation lands.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const I18N: &str = "docs/i18n";

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
            (p.starts_with("docs") && !p.starts_with(I18N))
                || ["README.md", "CHANGELOG.md", "brand/README.md"].contains(&p.to_str().unwrap())
        })
        .collect();
    assert!(docs.len() > 15 || tracked.is_none(), "found the docs: {docs:?}");
    for doc in docs {
        let text = std::fs::read_to_string(root().join(&doc)).unwrap();
        for target in targets(&text) {
            let path = resolve(doc.parent().unwrap(), target);
            if path.starts_with(I18N) {
                continue;
            }
            assert!(published(&path), "{}: links to {target}, which git doesn't track", doc.display());
        }
    }
}

/// A heading's anchor as GitHub makes it: lower case, spaces to `-`, other punctuation dropped.
fn slug(heading: &str) -> String {
    let text = heading.trim_start_matches('#').trim().to_lowercase();
    text.chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// The anchors of a Markdown file's headings (outside fenced blocks).
fn anchors(text: &str) -> BTreeSet<String> {
    let mut fenced = false;
    let mut out = BTreeSet::new();
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with('#') {
            out.insert(slug(line));
        }
    }
    out
}

#[test]
fn kept_docs_anchors_name_a_heading() {
    let docs: Vec<PathBuf> = match tracked() {
        Some(t) => t.into_iter().filter(|p| p.extension().is_some_and(|e| e == "md")).collect(),
        None => return,
    };
    let docs: Vec<PathBuf> = docs
        .into_iter()
        .filter(|p| {
            (p.starts_with("docs") && !p.starts_with(I18N))
                || ["README.md", "CHANGELOG.md", "brand/README.md"].contains(&p.to_str().unwrap())
        })
        .collect();
    let mut checked = 0;
    for doc in &docs {
        let text = std::fs::read_to_string(root().join(doc)).unwrap();
        for link in text.split("](").skip(1).map(|s| s.split(')').next().unwrap()) {
            let Some((file, anchor)) = link.split_once('#') else { continue };
            if link.contains("://") || !(file.is_empty() || file.ends_with(".md")) {
                continue;
            }
            let target = if file.is_empty() { doc.clone() } else { resolve(doc.parent().unwrap(), file) };
            if target.starts_with(I18N) {
                continue;
            }
            let heads = anchors(&std::fs::read_to_string(root().join(&target)).unwrap());
            assert!(
                heads.contains(anchor),
                "{}: links to {link}, but {} has no heading #{anchor}",
                doc.display(),
                target.display()
            );
            checked += 1;
        }
    }
    assert!(checked >= 10, "found the anchored links: {checked}");
}

/// CHANGELOG.md's [Unreleased] is one bullet per feature, each linking its detail under
/// docs/changelog-detail.md's "Since 0.1.0" heading, so the release notes name every feature and
/// the changelog stays small. A release renames [Unreleased] and leaves it empty, so the
/// bullets may be in a version's section by then.
#[test]
fn unreleased_bullets_link_to_their_detail() {
    let changelog = std::fs::read_to_string(root().join("CHANGELOG.md")).unwrap();
    assert!(changelog.len() < 10 * 1024, "CHANGELOG.md is {} bytes, over 10 KB", changelog.len());
    let unreleased = changelog.split("\n## [Unreleased]\n").nth(1).unwrap_or("").split("\n## [").next().unwrap();
    let detail = std::fs::read_to_string(root().join("docs/changelog-detail.md")).unwrap();
    let since = detail.split("\n## Since 0.1.0: the detail\n").nth(1).expect("a Since 0.1.0 heading");
    let since = since.split("\n## ").next().unwrap();
    let features: BTreeSet<String> = since.lines().filter(|l| l.starts_with("### ")).map(slug).collect();
    let heads = anchors(&detail);
    let link = "](docs/changelog-detail.md#";
    for bullet in unreleased.split("\n- ").skip(1) {
        let anchor = bullet.split(link).nth(1).and_then(|s| s.split(')').next());
        let anchor = anchor.unwrap_or_else(|| panic!("[Unreleased] bullet links no detail: - {bullet}"));
        assert!(heads.contains(anchor), "docs/changelog-detail.md has no heading #{anchor}");
        assert!(features.contains(anchor), "#{anchor} is not a feature under Since 0.1.0");
    }
    let linked: BTreeSet<String> =
        changelog.split(link).skip(1).filter_map(|s| s.split(')').next()).map(String::from).collect();
    assert_eq!(linked, features, "every feature under Since 0.1.0 has a bullet in CHANGELOG.md");
}
