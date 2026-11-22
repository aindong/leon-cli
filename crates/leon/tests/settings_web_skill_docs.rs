//! The docs say what Settings now does with the web page and the agent skill: docs/web.md's "Turn
//! it on" names the switches, the address and the QR code; docs/agents.md's "Set it up once" names
//! Settings → Agents and the notice; docs/cli/server.md has `--qr`; CHANGELOG.md's [Unreleased]
//! links the detail.

fn read(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)).unwrap()
}

/// The text of `doc`'s section `heading` (up to the next heading of its level or above).
fn section(doc: &str, heading: &str) -> String {
    let start = doc.find(&format!("\n{heading}\n")).unwrap_or_else(|| panic!("no {heading}"));
    let level = heading.split(' ').next().unwrap();
    let rest = &doc[start + heading.len() + 2..];
    let end = rest
        .match_indices("\n#")
        .find(|(i, _)| rest[i + 1..].split(' ').next().is_some_and(|h| h.len() <= level.len()))
        .map_or(rest.len(), |(i, _)| i);
    rest[..end].to_string()
}

#[test]
fn web_md_turn_it_on_names_the_settings_and_the_qr_code() {
    let s = section(&read("docs/web.md"), "## Turn it on");
    for w in ["Settings", "Tailscale access", "Web page", "Copy", "QR code", "--qr"] {
        assert!(s.contains(w), "docs/web.md's Turn it on says {w:?}:\n{s}");
    }
}

#[test]
fn agents_md_set_it_up_once_names_the_settings_section_and_the_notice() {
    let s = section(&read("docs/agents.md"), "## Set it up once");
    for w in ["Settings", "Agents", "Install", "Update", "Remove", "notice", "once"] {
        assert!(s.contains(w), "docs/agents.md's Set it up once says {w:?}");
    }
}

#[test]
fn the_cli_reference_and_the_changelog_have_it() {
    assert!(read("docs/cli/server.md").contains("- `--qr`:"), "docs/cli/server.md documents --qr");
    let changelog = read("CHANGELOG.md");
    let unreleased = section(&changelog, "## [Unreleased]");
    let link = "docs/changelog-detail.md#the-web-pages-address-and-the-agent-skill-in-settings";
    assert!(unreleased.contains(link), "an [Unreleased] bullet links {link}:\n{unreleased}");
    let detail = read("docs/changelog-detail.md");
    assert!(detail.contains("\n### The web page's address and the agent skill in Settings\n"), "its detail");
}
