//! `serve_tailscale`, `serve_tailscale_port` and `serve_tailscale_tags` (docs/lua/settings.md):
//! off, 7297 and none by default; a port is only a port, a tag only `tag:NAME`.

use serde_json::json;

fn load(src: &str) -> (leon_lua::Config, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    (leon_lua::load(&path).unwrap(), dir)
}

#[test]
fn serving_is_off_by_default() {
    let (cfg, _d) = load("");
    let s = &cfg.settings;
    assert!(!s.serve_tailscale);
    assert_eq!((s.serve_tailscale_port, s.serve_tailscale_tags.len()), (7297, 0));
    assert_eq!(s.to_json()["serve_tailscale_tags"], json!([]));
}

#[test]
fn only_a_port_and_only_tags_are_taken() {
    let mut s = leon_lua::Settings::default();
    let themes = std::path::Path::new("/nonexistent");
    for bad in [json!("0.0.0.0:7297"), json!(0), json!(65536), json!(7.5), json!("7297")] {
        assert!(s.set("serve_tailscale_port", &bad, themes).is_err(), "{bad}");
    }
    s.set("serve_tailscale_port", &json!(8022), themes).unwrap();
    assert_eq!(s.serve_tailscale_port, 8022);
    let many: Vec<String> = (0..17).map(|i| format!("tag:t{i}")).collect();
    for bad in [json!(["ci"]), json!(["tag:"]), json!(["tag:a b"]), json!(["tag:*"]), json!("tag:ci"), json!(many)] {
        assert!(s.set("serve_tailscale_tags", &bad, themes).is_err(), "{bad}");
    }
    s.set("serve_tailscale_tags", &json!(["tag:ci", "tag:build-1"]), themes).unwrap();
    assert_eq!(s.serve_tailscale_tags, ["tag:ci", "tag:build-1"]);
    assert!(s.set("serve_tailscale", &json!("yes"), themes).is_err());
}
