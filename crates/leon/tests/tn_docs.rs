//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 11: serving is documented
//! where people look, with its threat model, under the pages' size cap.

fn doc(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn serving_over_tailscale_is_documented() {
    let hosts = doc("docs/cli/hosts.md");
    for s in ["## Serve over Tailscale", "tailnet://", "serve_tailscale", "same user", "leon server status"] {
        assert!(hosts.contains(s), "docs/cli/hosts.md: {s}");
    }
    for page in ["docs/lua/settings.md", "docs/cli/settings.md"] {
        let d = doc(page);
        for k in ["serve_tailscale", "serve_tailscale_port", "serve_tailscale_tags"] {
            assert!(d.contains(k), "{page}: {k}");
        }
    }
    let arch = doc("docs/architecture/tailnet.md");
    for s in ["whois", "Threat model", "fd7a:115c:a1e0::/48", "LEON_TEST_TAILNET", "sameuserproof"] {
        assert!(arch.contains(s), "docs/architecture/tailnet.md: {s}");
    }
    assert!(arch.len() < 20_000, "under 20 KB: {}", arch.len());
    assert!(doc("docs/architecture.md").contains("architecture/tailnet.md"), "linked from the index");
    let hosts_arch = doc("docs/architecture/hosts.md");
    assert!(
        !hosts_arch.contains("Nothing listens on TCP"),
        "docs/architecture/hosts.md no longer says nothing listens"
    );
    // with the earlier releases' sections, which moved to the archive
    assert!((doc("CHANGELOG.md") + &doc("docs/changelog-archive.md")).contains("serve_tailscale"));
}
