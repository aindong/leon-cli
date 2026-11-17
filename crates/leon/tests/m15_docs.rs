//! M15 criterion 7 (.agent/plans/2026-11-15-m15-linux-server-cli.md): the docs say how leon runs on
//! Linux: a platform matrix in the architecture, a Linux install in docs/install.md, and what `leon` does
//! without the app in docs/cli.md.

mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The text from `heading` to the next heading of the same or a higher level.
fn section(doc: &str, heading: &str) -> Option<String> {
    let level = heading.chars().take_while(|&c| c == '#').count();
    let start = doc.lines().position(|l| l.trim() == heading)?;
    let rest: Vec<&str> = doc.lines().skip(start + 1).collect();
    let end = rest
        .iter()
        .position(|l| l.starts_with('#') && l.chars().take_while(|&c| c == '#').count() <= level)
        .unwrap_or(rest.len());
    Some(rest[..end].join("\n"))
}

#[test]
fn the_architecture_has_a_platform_matrix() {
    let arch = doc("docs/architecture.md");
    let s = section(&arch, "## Platforms").expect("docs/architecture.md has a `## Platforms` section");
    let rows: Vec<&str> = s.lines().filter(|l| l.starts_with('|')).collect();
    assert!(rows.iter().any(|r| r.contains("macOS")) && rows.iter().any(|r| r.contains("Linux")), "a matrix: {s}");
    assert!(s.contains("`app`"), "names the `app` feature: {s}");
}

#[test]
fn the_readme_says_how_to_install_on_linux() {
    let install = doc("docs/install.md");
    let s = section(&install, "## Linux").expect("docs/install.md has a `## Linux` install section");
    for word in ["tar", "dpkg", ".deb", "cargo"] {
        assert!(s.contains(word), "the Linux install mentions {word}: {s}");
    }
}

#[test]
fn cli_md_says_what_a_build_without_the_app_does() {
    let cli = doc("docs/cli.md");
    let s = section(&cli, "### `leon app`").expect("docs/cli.md has `leon app`");
    assert!(s.contains("without the app"), "says what a build without the app does: {s}");
}
