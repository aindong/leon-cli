//! M22a criterion 6 (.agent/plans/2026-11-15-m22a-themes-settings.md): `leon.set` takes the theme,
//! font, notification and copy settings, validates each, and records where each was set.

use leon_lua::{Level, Resume};

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
fn defaults_when_nothing_is_set() {
    let (cfg, _d) = load("");
    let s = &cfg.settings;
    assert_eq!((s.theme.dark.as_str(), s.theme.light.as_str()), ("leon-dark", "leon-dark"));
    assert_eq!((s.font_family.as_str(), s.font_size), ("Menlo", 13.0));
    assert!(s.notifications.done && s.notifications.blocked && s.notifications.error);
    assert!(!s.copy_on_select);
    assert_eq!(s.resume, Resume::Ask);
    assert!(cfg.setting_sources.is_empty());
}

#[test]
fn every_key_sets_and_records_its_line() {
    let (cfg, _d) = load(
        "-- my settings\nleon.set(\"theme\", \"leon-light\")\nleon.set(\"font.family\", \"JetBrains Mono\")\n\
         leon.set(\"font.size\", 15)\nleon.set(\"notifications.done\", false)\nleon.set(\"copy_on_select\", true)\n\
         leon.set(\"resume\", \"never\")\n",
    );
    assert!(errors(&cfg).is_empty(), "{:?}", errors(&cfg));
    let s = &cfg.settings;
    assert_eq!((s.theme.dark.as_str(), s.theme.light.as_str()), ("leon-light", "leon-light"));
    assert_eq!((s.font_family.as_str(), s.font_size), ("JetBrains Mono", 15.0));
    assert!(!s.notifications.done && s.notifications.blocked);
    assert!(s.copy_on_select);
    assert_eq!(s.resume, Resume::Never);
    assert!(cfg.setting_sources["font.size"].ends_with("init.lua:4"), "{:?}", cfg.setting_sources);
    assert_eq!(s.get("font.size"), Some(serde_json::json!(15.0)));
    assert_eq!(s.get("theme"), Some(serde_json::json!({ "dark": "leon-light", "light": "leon-light" })));
    assert_eq!(s.get("nope"), None);
}

#[test]
fn theme_follows_the_os_with_a_table() {
    let (cfg, _d) = load("leon.set(\"theme\", { dark = \"leon-dark\", light = \"leon-light\" })\n");
    assert!(errors(&cfg).is_empty(), "{:?}", errors(&cfg));
    assert_eq!((cfg.settings.theme.dark.as_str(), cfg.settings.theme.light.as_str()), ("leon-dark", "leon-light"));
}

#[test]
fn a_stored_theme_is_a_known_name() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("themes")).unwrap();
    std::fs::write(dir.path().join("themes/Harbor.theme"), "background = #000000\nforeground = #ffffff\n").unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, "leon.set(\"theme\", \"Harbor\")\n").unwrap();
    let cfg = leon_lua::load(&path).unwrap();
    assert!(errors(&cfg).is_empty(), "{:?}", errors(&cfg));
    assert_eq!(cfg.settings.theme.dark, "Harbor");
}

#[test]
fn bad_values_are_skipped_with_a_message() {
    for (call, want) in [
        ("leon.set(\"theme\", \"no-such-theme\")", "no-such-theme"),
        ("leon.set(\"theme\", 3)", "theme"),
        ("leon.set(\"theme\", { dark = \"leon-dark\" })", "light"),
        ("leon.set(\"font.size\", 99)", "8"),
        ("leon.set(\"font.size\", \"big\")", "font.size"),
        ("leon.set(\"font.family\", \"\")", "font.family"),
        ("leon.set(\"notifications.done\", \"no\")", "true or false"),
        ("leon.set(\"notifications.sometimes\", true)", "notifications.sometimes"),
        ("leon.set(\"copy_on_select\", 1)", "true or false"),
        ("leon.set(\"colour\", \"red\")", "unknown setting"),
    ] {
        let (cfg, _d) = load(&format!("{call}\n"));
        let errs = errors(&cfg);
        assert!(errs.len() == 1 && errs[0].contains(want), "{call}: {errs:?} should mention {want:?}");
        assert_eq!(cfg.settings, leon_lua::Settings::default(), "{call}: a skipped call changes nothing");
        assert!(cfg.setting_sources.is_empty(), "{call}");
    }
}
