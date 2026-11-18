//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criteria 10–11: the
//! docs say how to make and renew the files, the settings are listed, and building needs no
//! cmake or NASM (rustls with ring, never aws-lc).

fn doc(path: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn https_is_documented() {
    let page = doc("docs/web.md");
    for s in ["tailscale cert --cert-file", "serve_web_cert", "serve_web_key", "90 days", "https://"] {
        assert!(page.contains(s), "docs/web.md: {s}");
    }
    for p in ["docs/lua/settings.md", "docs/cli/settings.md"] {
        for k in ["serve_web_cert", "serve_web_key"] {
            assert!(doc(p).contains(k), "{p}: {k}");
        }
    }
    let arch = doc("docs/architecture/web.md");
    for s in ["ring", "rustls", "web_error", "never runs `tailscale cert`"] {
        assert!(arch.contains(s), "docs/architecture/web.md: {s}");
    }
    assert!(doc("CHANGELOG.md").contains("HTTPS"));
}

#[test]
fn building_needs_no_aws_lc() {
    let lock = doc("Cargo.lock");
    assert!(!lock.contains("name = \"aws-lc-sys\""), "rustls is built with ring only");
}
