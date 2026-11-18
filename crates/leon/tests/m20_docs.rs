#[test]
fn m20_documented_contract() {
    // float, toggle and dock moved to docs/cli/agents.md (file-size limit)
    let cli = [include_str!("../../../docs/cli.md"), include_str!("../../../docs/cli/agents.md")].concat();
    for text in ["### `leon float", "### `leon toggle", "### `leon dock", "--rect", "--unfloat", "--size", "--create"] {
        assert!(cli.contains(text), "missing {text}");
    }
    let lua = include_str!("../../../docs/lua/keys.md");
    for text in ["leon.float", "leon.toggle", "leon.dock", "<!-- m20-scratch -->"] {
        assert!(lua.contains(text), "missing {text}");
    }
    let arch = include_str!("../../../docs/architecture/protocol-and-clients.md");
    for text in ["Session.overlays", "session_view_changed", "M20b", "back to front"] {
        assert!(arch.contains(text), "missing {text}");
    }
    assert!(include_str!("../../../CHANGELOG.md").contains(leon_server::API_VERSION));
    let minor: u64 = leon_server::API_VERSION.split('.').nth(1).unwrap().parse().unwrap();
    assert!(minor >= 2);
}
