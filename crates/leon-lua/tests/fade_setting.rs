//! `leon.set("fade_inactive_panes", N)`: how far the app fades the panes without the focus, 0 to
//! 0.8, 0 for off. A value outside that is a skipped call with a message that says the range.

use leon_lua::settings::{FADE, FADE_MAX, KEYS};
use leon_lua::Level;
use serde_json::json;

fn load(src: &str) -> (leon_lua::Config, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    (leon_lua::load(&path).unwrap(), dir)
}

fn errors(cfg: &leon_lua::Config) -> Vec<String> {
    cfg.diagnostics.iter().filter(|d| d.level == Level::Error).map(|d| d.to_string()).collect()
}

#[test]
fn the_default_is_on_and_in_range() {
    let (cfg, _d) = load("");
    assert_eq!(cfg.settings.fade_inactive_panes, FADE);
    let on = cfg.settings.fade_inactive_panes;
    assert!(on > 0. && on <= FADE_MAX, "the fade is on by default, and in range: {on}");
    assert_eq!(FADE_MAX, 0.8);
    assert!(KEYS.contains(&"fade_inactive_panes"));
    assert_eq!(cfg.settings.get("fade_inactive_panes"), Some(json!(FADE)));
    assert_eq!(cfg.settings.to_json()["fade_inactive_panes"], json!(FADE));
}

#[test]
fn a_strength_in_range_is_taken_and_0_is_off() {
    for n in [0., 0.05, 0.35, 0.8] {
        let (cfg, _d) = load(&format!("leon.set(\"fade_inactive_panes\", {n})\n"));
        assert!(errors(&cfg).is_empty(), "{n}: {:?}", errors(&cfg));
        assert_eq!(cfg.settings.fade_inactive_panes, n);
        assert!(cfg.setting_sources["fade_inactive_panes"].ends_with("init.lua:1"), "{n}");
    }
}

#[test]
fn a_value_out_of_range_is_refused_and_says_the_range() {
    for bad in ["0.81", "1", "2", "-0.1", "\"strong\"", "true", "false", "{}", "0/0"] {
        let (cfg, _d) = load(&format!("leon.set(\"fade_inactive_panes\", {bad})\n"));
        let errs = errors(&cfg);
        assert_eq!(errs.len(), 1, "{bad}: {errs:?}");
        for word in ["fade_inactive_panes", "0", "0.8", "off"] {
            assert!(errs[0].contains(word), "{bad}: {:?} should say {word:?}", errs[0]);
        }
        assert_eq!(cfg.settings, leon_lua::Settings::default(), "{bad}: a skipped call changes nothing");
        assert!(cfg.setting_sources.is_empty(), "{bad}");
    }
}
