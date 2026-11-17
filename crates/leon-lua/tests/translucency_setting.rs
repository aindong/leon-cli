//! `leon.set("translucency", N)`: how much of the desktop shows through Leon.app's window, 0
//! (opaque) to 0.8. A value outside that is a skipped call with a message that says the range.

use leon_lua::settings::{KEYS, TRANSLUCENCY, TRANSLUCENCY_MAX};
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
fn the_default_is_translucent_and_in_range() {
    let (cfg, _d) = load("");
    assert_eq!(cfg.settings.translucency, TRANSLUCENCY);
    let on = cfg.settings.translucency;
    assert!(on > 0. && on <= TRANSLUCENCY_MAX, "translucent by default, and in range: {on}");
    assert_eq!(TRANSLUCENCY_MAX, 0.8);
    assert!(KEYS.contains(&"translucency"));
    assert_eq!(cfg.settings.get("translucency"), Some(json!(TRANSLUCENCY)));
    assert_eq!(cfg.settings.to_json()["translucency"], json!(TRANSLUCENCY));
}

#[test]
fn a_value_in_range_is_taken_and_0_is_opaque() {
    for n in [0., 0.05, 0.3, 0.8] {
        let (cfg, _d) = load(&format!("leon.set(\"translucency\", {n})\n"));
        assert!(errors(&cfg).is_empty(), "{n}: {:?}", errors(&cfg));
        assert_eq!(cfg.settings.translucency, n);
        assert!(cfg.setting_sources["translucency"].ends_with("init.lua:1"), "{n}");
    }
}

#[test]
fn a_value_out_of_range_is_refused_and_says_the_range() {
    for bad in ["0.81", "1", "-0.1", "\"glass\"", "true", "{}", "0/0"] {
        let (cfg, _d) = load(&format!("leon.set(\"translucency\", {bad})\n"));
        let errs = errors(&cfg);
        assert_eq!(errs.len(), 1, "{bad}: {errs:?}");
        for word in ["translucency", "0", "0.8", "opaque"] {
            assert!(errs[0].contains(word), "{bad}: {:?} should say {word:?}", errs[0]);
        }
        assert_eq!(cfg.settings, leon_lua::Settings::default(), "{bad}: a skipped call changes nothing");
        assert!(cfg.setting_sources.is_empty(), "{bad}");
    }
}
