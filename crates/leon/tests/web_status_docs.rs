//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criteria 11–12: `leon server
//! status` names the page's URL, the API version moved, and the docs say how and who.

mod support;

use support::localapi::Serving;

fn doc(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn server_status_prints_the_web_url() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    s.until("the web URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    let out = s.cmd().args(["server", "status"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("web      http://127.0.0.1:{}/", s.port)), "{text}");
    let v = s.env.json(&["api", "call", "server.version"]);
    let api: Vec<u64> = v["api_version"].as_str().unwrap().split('.').map(|n| n.parse().unwrap()).collect();
    assert!(api[0] == 1 && api[1] >= 13, "a minor bump for serve_web and tailnet.web: {v}");
}

#[test]
fn the_web_client_is_documented() {
    let page = doc("docs/web.md");
    for s in ["serve_web", "serve_tailscale", "Origin", "token", "HTTPS", "clipboard", "leon server status"] {
        assert!(page.contains(s), "docs/web.md: {s}");
    }
    assert!(doc("docs/remote-hosts.md").contains("](web.md)"), "linked from remote-hosts.md");
    for p in ["docs/lua/settings.md", "docs/cli/settings.md"] {
        assert!(doc(p).contains("serve_web"), "{p}");
    }
    let arch = doc("docs/architecture/web.md");
    for s in ["Threat model", "DNS rebinding", "Origin", "SameSite", "cert", "serve::connection"] {
        assert!(arch.contains(s), "docs/architecture/web.md: {s}");
    }
    assert!(arch.len() < 20_000, "under 20 KB: {}", arch.len());
    assert!(doc("docs/architecture.md").contains("architecture/web.md"), "linked from the index");
    assert!(
        !doc("docs/architecture/protocol-and-clients.md").contains("which leon doesn't serve"),
        "the ADR no longer says leon serves no WebSocket"
    );
    assert!(doc("CHANGELOG.md").contains("serve_web"));
}
