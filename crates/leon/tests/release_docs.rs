//! Release pipeline criterion 12: docs/releases.md says how a release is cut, signed and checked,
//! and the README's Install section sends people to the releases and the Homebrew tap.

mod release_kit;

use release_kit::repo;

fn read(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn releases_doc_covers_cutting_signing_the_manifest_and_the_tap() {
    let doc = read("docs/releases.md");
    assert!(doc.len() < 20_000, "docs/releases.md stays under 20 KB: {}", doc.len());
    for heading in ["## Cutting a release", "## Secrets", "## Signatures", "## The manifest"].into_iter().chain([
        "## Key rotation",
        "## Homebrew",
        "## Developer ID",
        "## Checking a download",
    ]) {
        assert!(doc.lines().any(|l| l == heading), "docs/releases.md has {heading}");
    }
    for needle in [
        "LEON_RELEASE_KEY",
        "HOMEBREW_TAP_TOKEN",
        "crates/leon/release-keys.pub",
        "https://github.com/aindong/leon-cli/releases/latest/download/latest.json",
        "minisign-verify",
        "https://docs.brew.sh/Acceptable-Casks",
        "xattr -dr com.apple.quarantine",
        "macos-15-intel",
        "workflow_dispatch",
        "Fine-grained",
        "aindong/homebrew-leon",
        // the branch flow: work lands on dev, a release is cut from main
        "pull request from `dev` to `main`",
        "## Cutting a release",
        "### When a run fails",
        "is not on main",
    ] {
        assert!(doc.contains(needle), "docs/releases.md mentions {needle}");
    }
    // the failures it explains are the scripts' own words
    let scripts: String = ["release-version.sh", "release-verify.sh"]
        .iter()
        .map(|s| std::fs::read_to_string(repo().join("scripts").join(s)).unwrap())
        .collect();
    for said in ["is not on main", "pins no key", "its signature is not by a key in"] {
        assert!(doc.contains(said) && scripts.contains(said), "{said:?} is in the doc and in a script");
    }
    assert!(read(".github/workflows/release.yml").contains("exists already") && doc.contains("exists already"));

    // every release script is described, and every script the doc names exists
    for e in std::fs::read_dir(repo().join("scripts")).unwrap() {
        let name = e.unwrap().file_name().into_string().unwrap();
        if name.starts_with("release") {
            assert!(doc.contains(&format!("scripts/{name}")), "docs/releases.md describes scripts/{name}");
        }
    }
    for word in doc.split(|c: char| !(c.is_ascii_alphanumeric() || "/-._".contains(c))) {
        if word.starts_with("scripts/") && word.ends_with(".sh") {
            assert!(repo().join(word).is_file(), "docs/releases.md names {word}, which does not exist");
        }
    }
}

#[test]
fn readme_install_covers_releases_and_homebrew() {
    let readme = read("README.md");
    let install = readme.split("\n## Install\n").nth(1).expect("an Install section").split("\n## ").next().unwrap();
    for needle in [
        "brew install --cask aindong/leon/leon",
        "brew install aindong/leon/leon",
        "https://github.com/aindong/leon-cli/releases/latest",
        "xattr -dr com.apple.quarantine",
        "docs/releases.md",
        "scripts/install.sh",
    ] {
        assert!(install.contains(needle), "the README's Install section has {needle:?}");
    }
    assert!(!readme.contains("no published release yet"), "the README no longer says there is no release");
    assert!(readme.contains("](docs/releases.md)"), "the README links docs/releases.md");
    assert!(read("CHANGELOG.md").contains(".github/workflows/release.yml"), "the changelog names the workflow");
    assert!(read("docs/architecture/state-and-hosts.md").contains("docs/releases.md"), "architecture points at it");
    assert!(read("CLAUDE.md").contains("docs/releases.md"), "CLAUDE.md points at it");
}
