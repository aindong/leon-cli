//! The demo's config lives in its temp HOME, so Settings names it from `~` (`~/.config/leon/
//! init.lua`) in a screenshot, never as a temp path outside it.

#[test]
fn every_demo_config_is_inside_the_temp_home() {
    let script = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/demo-shots.sh")).unwrap();
    let configs: Vec<&str> =
        script.split("LEON_CONFIG=").skip(1).map(|s| s.split_whitespace().next().unwrap()).collect();
    assert!(!configs.is_empty(), "the script sets LEON_CONFIG");
    for c in &configs {
        assert!(c.starts_with("\"$home/"), "LEON_CONFIG={c} is outside the demo's HOME");
    }
    assert!(script.contains("cat >\"$home/.config/leon/init.lua\""), "the config is written there");
    assert!(script.contains("config check \"$home/.config/leon/init.lua\""), "and checked there");
    assert!(!script.contains("$d/init.lua"), "no config left outside HOME");
}
