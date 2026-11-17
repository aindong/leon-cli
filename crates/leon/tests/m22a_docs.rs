//! M22a criterion 8 (.agent/plans/2026-11-15-m22a-themes-settings.md): the docs cover the theme and
//! settings commands, every setting key, the new method and the API bump.

#[test]
fn m22a_documented_contract() {
    let cli = include_str!("../../../docs/cli/settings.md");
    for text in [
        "### `leon theme list`",
        "### `leon theme import FILE",
        "### `leon theme show NAME`",
        "### `leon config get",
        "### `leon config set KEY VALUE`",
        ".itermcolors",
    ] {
        assert!(cli.contains(text), "docs/cli/settings.md misses {text}");
    }
    let lua = include_str!("../../../docs/lua/settings.md");
    for key in leon_lua::settings::KEYS {
        assert!(lua.contains(&format!("`{key}`")), "docs/lua/settings.md doesn't document the setting {key}");
    }
    assert!(lua.contains("settings_changed") && lua.contains("server.settings"), "docs/lua/settings.md");
    let changelog = include_str!("../../../docs/changelog-milestones.md");
    assert!(changelog.contains("M22a") && changelog.contains("server.settings"), "the milestones' changelog");
    let minor: u64 = leon_server::API_VERSION.split('.').nth(1).unwrap().parse().unwrap();
    assert!(minor >= 3, "server.settings is a new stable method: a minor bump");
    assert!(leon_server::contract::named().any(|m| m == "server.settings"), "contract.rs lists server.settings");
    assert!(include_str!("../../../docs/api.md").contains("server.settings"), "docs/api.md is regenerated");
}
