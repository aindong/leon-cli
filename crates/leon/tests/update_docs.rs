//! Update criterion 20 (.agent/plans/2026-11-16-self-update-core.md): the docs say how updating
//! works, in the updater's own words, and `docs/cli.md` still fits its cap with the new command.

mod release_kit;

use release_kit::repo;

fn read(path: &str) -> String {
    std::fs::read_to_string(repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The text under `heading` up to the next heading of the same or a higher level.
fn section<'a>(doc: &'a str, heading: &str) -> &'a str {
    let at = doc.find(&format!("\n{heading}\n")).unwrap_or_else(|| panic!("no section {heading:?}"));
    let rest = &doc[at + 1..];
    let level = heading.chars().take_while(|c| *c == '#').count();
    let end = rest.lines().skip(1).find(|l| {
        let n = l.chars().take_while(|c| *c == '#').count();
        n > 0 && n <= level && l[n..].starts_with(' ')
    });
    end.map_or(rest, |l| &rest[..rest.find(&format!("\n{l}\n")).unwrap()])
}

#[test]
fn cli_md_has_the_command_and_fits_its_cap() {
    let cli = read("docs/cli.md");
    assert!(cli.len() < 20_000, "docs/cli.md stays under 20 KB: {}", cli.len());
    let own = section(&cli, "### `leon self-update`");
    for needle in ["--check", "10", "(updates.md)"] {
        assert!(own.contains(needle), "the self-update section has {needle:?}: {own}");
    }
    let codes = section(&cli, "## Exit codes");
    assert!(codes.lines().any(|l| l.starts_with("| 10 |") && l.contains("self-update --check")), "{codes}");
}

#[test]
fn updates_md_says_how_updating_works() {
    let doc = read("docs/updates.md");
    assert!(doc.len() < 20_000, "docs/updates.md stays under 20 KB: {}", doc.len());
    for heading in ["## What is checked", "## Homebrew and `.deb` installs", "## What leon never does"]
        .into_iter()
        .chain(["## A running server", "## Another place for releases", "## When it refuses"])
    {
        assert!(doc.lines().any(|l| l == heading), "docs/updates.md has {heading}");
    }
    for needle in [
        "leon self-update --check",
        "LEON_RELEASE_URL",
        "brew upgrade aindong/leon/leon",
        "brew upgrade --cask aindong/leon/leon",
        "dpkg -i",
        "crates/leon/release-keys.pub",
        "sha256",
        "minisign",
        "User-Agent",
        "leon server stop",
        "curl",
        "sudo",
        ".leon-update",
        "https://github.com/aindong/leon-cli/releases/latest",
        "(releases.md",
    ] {
        assert!(doc.contains(needle), "docs/updates.md mentions {needle:?}");
    }
    let never = section(&doc, "## What leon never does").to_lowercase();
    for needle in ["on its own", "telemetry", "password"] {
        assert!(never.contains(needle), "what leon never does: {needle:?}\n{never}");
    }
    // why pointing leon elsewhere is safe is said where the variable is
    let mirror = section(&doc, "## Another place for releases");
    assert!(mirror.contains("LEON_RELEASE_URL") && mirror.contains("signature"), "{mirror}");

    // the refusals it explains are the updater's own words
    let code: String = std::fs::read_dir(repo().join("crates/leon-update/src"))
        .expect("crates/leon-update/src")
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    let refuses = section(&doc, "## When it refuses");
    for said in [
        "does not match its signature",
        "a key this leon does not trust",
        "no release key",
        "too old to read this release",
        "can't write",
        "another update",
    ] {
        assert!(refuses.contains(said) && code.contains(said), "{said:?} is in the doc and in the code");
    }
}

#[test]
fn the_other_docs_point_at_it() {
    let releases = read("docs/releases.md");
    assert!(!releases.contains("Nothing in leon reads"), "docs/releases.md no longer says nothing reads the keys");
    assert!(releases.contains("(updates.md)"), "docs/releases.md links docs/updates.md");
    // 0.1.0's section moved to the archive, whose links start from docs/
    let changelog = read("docs/changelog-archive.md");
    let release = changelog.split("\n## [0.1.0] ").nth(1).expect("a 0.1.0 section");
    let release = release.split("\n## [").next().unwrap();
    assert!(release.contains("leon self-update") && release.contains("](updates.md)"), "the changelog has it");
    let readme = read("README.md");
    let install = readme.split("\n## Install\n").nth(1).expect("an Install section").split("\n## ").next().unwrap();
    for needle in ["leon self-update", "brew upgrade", "(docs/updates.md)"] {
        assert!(install.contains(needle), "the README's Install section has {needle:?}");
    }
    assert!(read("docs/roadmap.md").contains("leon self-update"), "the roadmap has the item");
    assert!(read("docs/architecture.md").contains("`leon-update`"), "architecture.md lists the crate");
    assert!(read("docs/architecture/state-and-hosts.md").contains("docs/updates.md"), "architecture points at it");
    let doc_commands = read("crates/leon/tests/m7b_doc_commands.rs");
    assert!(doc_commands.contains("\"docs/updates.md\""), "docs/updates.md's sh blocks are run");
    assert!(doc_commands.contains("LEON_RELEASE_URL"), "and a documented self-update cannot reach the network");
}
