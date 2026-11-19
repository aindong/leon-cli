//! The translucent window and its setting are documented where a user, an integrator or a
//! contributor would look, each case included.

#[test]
fn translucency_is_documented() {
    let lua = include_str!("../../../docs/lua/settings.md");
    assert!(lua.contains("`translucency`"), "docs/lua/settings.md lists the setting");
    assert!(lua.contains("leon.set(\"translucency\", 0)"), "docs/lua/settings.md shows how to make the window opaque");
    for case in ["Reduce Transparency", "full screen"] {
        assert!(lua.contains(case), "docs/lua/settings.md says what {case} does");
    }
    let cli = include_str!("../../../docs/cli/settings.md");
    assert!(cli.contains("translucency"), "docs/cli/settings.md names the key config get/set take");
    // with the earlier releases' sections, which moved to the archive
    let changelog = concat!(include_str!("../../../CHANGELOG.md"), include_str!("../../../docs/changelog-archive.md"));
    assert!(changelog.contains("translucency"), "CHANGELOG.md has the entry");
    assert!(changelog.contains(leon_server::API_VERSION), "CHANGELOG.md names the API version");
    let minor: u64 = leon_server::API_VERSION.split('.').nth(1).unwrap().parse().unwrap();
    assert!(minor >= 8, "a new field on server.settings is a minor bump");
    assert!(include_str!("../../../docs/api.md").contains("translucency"), "docs/api.md is regenerated");
    assert!(include_str!("../../../docs/integrators.md").contains(leon_server::API_VERSION), "docs/integrators.md");
    // the README's "Know what needs you" moved to docs/shortcuts.md (README size)
    let app = include_str!("../../../docs/shortcuts.md");
    assert!(app.contains("translucency"), "docs/shortcuts.md says it can be set");
    let gpui = include_str!("../../../docs/architecture/gpui.md");
    assert!(
        gpui.contains("accessibilityDisplayShouldReduceTransparency"),
        "the GPUI ADR lists the Reduce Transparency read outside GPUI"
    );
    let arch = include_str!("../../../docs/architecture/attention.md");
    for case in ["translucency", "inactive", "full screen", "Reduce Transparency", "imported"] {
        assert!(arch.contains(case), "the architecture says what {case} does");
    }
}
