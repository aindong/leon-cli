//! Publishing: the README a visitor lands on shows the banner and both recordings, covers what the
//! public needs (install, configuration, roadmap with the known limits, build, license), and names
//! no other terminal product.
use std::path::Path;

fn repo(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)
}

#[test]
fn readme_shows_the_banner_and_both_recordings() {
    let readme = std::fs::read_to_string(repo("README.md")).unwrap();
    let banner_at = readme.find("docs/images/banner.png").expect("the README shows the banner");
    assert!(banner_at < readme.find("\n## ").unwrap(), "the banner is at the top");
    let png = std::fs::read(repo("docs/images/banner.png")).unwrap();
    assert_eq!(&png[1..4], b"PNG", "the banner is a PNG (GitHub loads no web fonts inside an SVG)");
    let (w, h) =
        (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()));
    assert!((1200..=1400).contains(&w) && (350..=450).contains(&h), "about 1280x400: {w}x{h}");
    for gif in ["demo-app.gif", "demo-cli.gif"] {
        assert!(readme.contains(&format!("](docs/images/{gif})")), "the README shows {gif}");
        let bytes = std::fs::read(repo(&format!("docs/images/{gif}"))).unwrap();
        assert!(bytes.starts_with(b"GIF89a"), "{gif} is a GIF");
        assert!(bytes.len() < 8 << 20, "{gif} is under 8 MB: {}", bytes.len());
    }
}

#[test]
fn readme_covers_what_a_visitor_needs() {
    let readme = std::fs::read_to_string(repo("README.md")).unwrap();
    for heading in ["## Features", "## Install", "## The first five commands", "## App shortcuts", "## Configuration"]
        .into_iter()
        .chain(["## Roadmap", "## Docs", "## Build and test", "## License"])
    {
        assert!(readme.lines().any(|l| l == heading), "the README has {heading}");
    }
    assert!(readme.contains("scripts/release.sh"), "says what the release script makes");
    let roadmap = readme.split("## Roadmap").nth(1).unwrap().split("\n## ").next().unwrap();
    for limit in ["Linux", "leon attach", "Lookalike", "codex exec"] {
        assert!(roadmap.contains(limit), "the roadmap names the known limit {limit:?}");
    }
}

#[test]
fn readme_names_no_other_terminal_product() {
    let readme = std::fs::read_to_string(repo("README.md")).unwrap().to_lowercase();
    for name in ["tmux", "zellij", "wezterm", "iterm", "ghostty", "kitty", "alacritty", "warp"] {
        assert!(!readme.contains(name), "the README names {name}");
    }
}
