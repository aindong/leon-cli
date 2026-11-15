//! Publishing: every local link in the README, the changelog, brand/README.md and docs/ resolves,
//! and none points at the internal notes kept out of git (they stay on disk in a work tree, so a
//! link to one would still resolve there).
use std::path::{Path, PathBuf};

const UNTRACKED: [&str; 6] = ["docs/spec", "spec/", "docs/research", "research/", "audit-0.1.0", "exploration/"];

fn markdown(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        if path.is_dir() && !["spec", "research"].contains(&name) {
            markdown(&path, out);
        } else if name.ends_with(".md") && !name.starts_with("audit-") && name != "spec.md" {
            out.push(path);
        }
    }
}

/// The targets of `](target)` and `src="target"`, without their `#anchor`.
fn targets(text: &str) -> Vec<&str> {
    let md = text.split("](").skip(1).map(|s| s.split(')').next().unwrap());
    let html = text.split("src=\"").skip(1).map(|s| s.split('"').next().unwrap());
    md.chain(html).map(|t| t.split('#').next().unwrap()).filter(|t| !t.is_empty() && !t.contains("://")).collect()
}

#[test]
fn kept_docs_link_only_to_files_in_the_repo() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = vec![root.join("README.md"), root.join("CHANGELOG.md"), root.join("brand/README.md")];
    markdown(&root.join("docs"), &mut files);
    assert!(files.len() > 15, "found the docs: {files:?}");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for target in targets(&text) {
            assert!(file.parent().unwrap().join(target).exists(), "{}: missing {target}", file.display());
            for gone in UNTRACKED {
                assert!(!target.contains(gone), "{}: links to the untracked {target}", file.display());
            }
        }
    }
}
