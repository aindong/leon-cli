//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 10: server.status and
//! server.settings report serving, under a new minor api_version.

mod support;

use support::Env;

#[test]
fn status_and_settings_report_serving() {
    let env = Env::new();
    let v = env.json(&["api", "call", "server.version"]);
    let api: Vec<u64> = v["api_version"].as_str().unwrap().split('.').map(|n| n.parse().unwrap()).collect();
    assert!(api[0] == 1 && api[1] >= 9, "a minor bump for the new fields: {v}");
    let st = env.json(&["server", "status", "--json"]);
    let t = &st["tailnet"];
    for k in ["state", "port", "addresses", "login", "reason", "last_refusal"] {
        assert!(t.get(k).is_some(), "tailnet.{k}: {st}");
    }
    assert_eq!(t["state"], "off");
    let s = &env.json(&["api", "call", "server.settings"])["settings"];
    assert_eq!(s["serve_tailscale"], false, "{s}");
    assert_eq!(s["serve_tailscale_port"], 7297, "{s}");
    assert_eq!(s["serve_tailscale_tags"], serde_json::json!([]), "{s}");
    let api_md = std::fs::read_to_string(support_repo().join("docs/api.md")).unwrap();
    assert!(api_md.contains("last_refusal") && api_md.contains("serve_tailscale_port"), "docs/api.md regenerated");
}

fn support_repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
