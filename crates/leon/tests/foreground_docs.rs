//! The foreground program is documented where an integrator and a user would look.

#[test]
fn foreground_is_documented() {
    let integrators = include_str!("../../../docs/integrators.md");
    assert!(
        integrators.contains("`foreground_changed {foreground}`"),
        "docs/integrators.md names the event and its payload"
    );
    let changelog = include_str!("../../../CHANGELOG.md");
    assert!(changelog.contains("`foreground_changed`") && changelog.contains("never its arguments"), "CHANGELOG.md");
    assert!(changelog.contains(leon_server::API_VERSION), "CHANGELOG.md names the API version");
}
