//! Tailscale criterion 8 (.agent/plans/2026-11-16-tailscale-hosts.md): the feature is documented
//! where people look, and the command reference keeps under its cap.

mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn tailscale_is_documented() {
    let cli = doc("docs/cli.md");
    assert!(cli.len() < 20_000, "docs/cli.md stays under 20 KB: {}", cli.len());
    assert!(cli.contains("cli/hosts.md"), "docs/cli.md links the hosts page");
    let hosts = doc("docs/cli/hosts.md");
    for s in ["### `leon hosts add", "--tailscale", "LEON_TAILSCALE", "Tailscale SSH", "status --json"] {
        assert!(hosts.contains(s), "docs/cli/hosts.md: {s}");
    }
    let readme = doc("README.md");
    assert!(readme.contains("leon hosts add --tailscale"), "README");
    assert!(readme.contains("Tailscale SSH"), "README");
    let arch = doc("docs/architecture/hosts.md");
    assert!(arch.lines().any(|l| l.starts_with('#') && l.contains("Tailscale")), "a Tailscale section");
    for s in ["LEON_TAILSCALE", "status --json", "sshHostKeys", "hosts.json"] {
        assert!(arch.contains(s), "docs/architecture/hosts.md: {s}");
    }
    assert!(arch.len() < 20_000);
}
