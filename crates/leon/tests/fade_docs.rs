//! The fade and its setting are documented where a user or an integrator would look.

#[test]
fn the_fade_is_documented() {
    let lua = include_str!("../../../docs/lua.md");
    assert!(lua.contains("`fade_inactive_panes`"), "docs/lua.md lists the setting");
    assert!(lua.contains("leon.set(\"fade_inactive_panes\", 0)"), "docs/lua.md shows how to turn the fade off");
    let cli = include_str!("../../../docs/cli/settings.md");
    assert!(cli.contains("fade_inactive_panes"), "docs/cli/settings.md names the key config get/set take");
    let changelog = include_str!("../../../CHANGELOG.md");
    assert!(changelog.contains("fade_inactive_panes"), "CHANGELOG.md has the entry");
    assert!(changelog.contains(leon_server::API_VERSION), "CHANGELOG.md names the API version");
    let minor: u64 = leon_server::API_VERSION.split('.').nth(1).unwrap().parse().unwrap();
    assert!(minor >= 6, "a new field on server.settings is a minor bump");
    let api = include_str!("../../../docs/api.md");
    assert!(api.contains("fade_inactive_panes"), "docs/api.md is regenerated");
    assert!(include_str!("../../../docs/integrators.md").contains(leon_server::API_VERSION), "docs/integrators.md");
    let readme = include_str!("../../../README.md");
    assert!(readme.contains("fade_inactive_panes"), "the README says the fade can be set or turned off");
    let arch = include_str!("../../../docs/architecture/attention.md");
    assert!(arch.contains("fade_inactive_panes"), "the architecture says how the app fades panes");
}
