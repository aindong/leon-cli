//! `leon.set("cursor_shader", PATH)` and `leon.set("cursor_shader_duration", SECS)`: a
//! Shadertoy-style cursor shader file for Leon.app (docs/cursor-shaders.md), and how long it
//! animates after the cursor changes. A bad value is a skipped call that says what it takes.

use leon_lua::settings::{CURSOR_SHADER_DURATION, CURSOR_SHADER_DURATIONS, KEYS};
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
fn off_by_default_and_reported() {
    let (cfg, _d) = load("");
    assert_eq!(cfg.settings.cursor_shader, "");
    assert_eq!(cfg.settings.cursor_shader_duration, CURSOR_SHADER_DURATION);
    assert_eq!(CURSOR_SHADER_DURATION, 1.0);
    assert_eq!(CURSOR_SHADER_DURATIONS, (0.1, 5.0));
    for k in ["cursor_shader", "cursor_shader_duration"] {
        assert!(KEYS.contains(&k), "{k} is a setting");
    }
    assert_eq!(cfg.settings.get("cursor_shader"), Some(json!("")));
    assert_eq!(cfg.settings.to_json()["cursor_shader_duration"], json!(1.0));
}

#[test]
fn a_path_and_a_duration_in_range_are_taken() {
    let src = "leon.set(\"cursor_shader\", \"~/shaders/smear.glsl\")\nleon.set(\"cursor_shader_duration\", 0.5)\n";
    let (cfg, _d) = load(src);
    assert!(errors(&cfg).is_empty(), "{:?}", errors(&cfg));
    assert_eq!(cfg.settings.cursor_shader, "~/shaders/smear.glsl", "kept as written: the app expands ~");
    assert_eq!(cfg.settings.cursor_shader_duration, 0.5);
    assert!(cfg.setting_sources["cursor_shader"].ends_with("init.lua:1"));
    for n in [0.1, 5.0] {
        let (cfg, _d) = load(&format!("leon.set(\"cursor_shader_duration\", {n})\n"));
        assert!(errors(&cfg).is_empty(), "{n}: {:?}", errors(&cfg));
        assert_eq!(cfg.settings.cursor_shader_duration, n);
    }
    let (cfg, _d) = load("leon.set(\"cursor_shader\", \"\")\n");
    assert!(errors(&cfg).is_empty(), "\"\" turns it off: {:?}", errors(&cfg));
}

#[test]
fn bad_values_are_refused_and_say_what_they_take() {
    let bad = [
        ("cursor_shader_duration", "0.05", ["cursor_shader_duration", "0.1", "5"]),
        ("cursor_shader_duration", "6", ["cursor_shader_duration", "0.1", "5"]),
        ("cursor_shader_duration", "\"long\"", ["cursor_shader_duration", "0.1", "5"]),
        ("cursor_shader", "42", ["cursor_shader", "path", "\"\""]),
        ("cursor_shader", "true", ["cursor_shader", "path", "\"\""]),
        ("cursor_shader", "\"a\\nb\"", ["cursor_shader", "path", "\"\""]),
        ("cursor_shader", &format!("\"{}\"", "x".repeat(1025)), ["cursor_shader", "path", "\"\""]),
    ];
    for (key, v, words) in bad {
        let (cfg, _d) = load(&format!("leon.set(\"{key}\", {v})\n"));
        let errs = errors(&cfg);
        assert_eq!(errs.len(), 1, "{key} = {v}: {errs:?}");
        for w in words {
            assert!(errs[0].contains(w), "{key} = {v}: {:?} should say {w:?}", errs[0]);
        }
        assert_eq!(cfg.settings, leon_lua::Settings::default(), "{key} = {v}: a skipped call changes nothing");
    }
}

#[test]
fn the_path_resolves_home_and_the_config_folder() {
    use leon_lua::settings::shader_path;
    use std::path::{Path, PathBuf};
    let (cfg, home) = (Path::new("/c/leon/init.lua"), Some(Path::new("/h")));
    assert_eq!(shader_path("", cfg, home), None, "empty is no shader");
    assert_eq!(shader_path("  ", cfg, home), None);
    assert_eq!(shader_path("~/s/smear.glsl", cfg, home), Some(PathBuf::from("/h/s/smear.glsl")));
    assert_eq!(shader_path("/abs/x.glsl", cfg, home), Some(PathBuf::from("/abs/x.glsl")));
    assert_eq!(shader_path("shaders/x.glsl", cfg, home), Some(PathBuf::from("/c/leon/shaders/x.glsl")));
    // no HOME: `~/x` is a name in the config's folder, not a guess at someone's home
    assert_eq!(shader_path("~/x.glsl", cfg, None), Some(PathBuf::from("/c/leon/~/x.glsl")));
}
