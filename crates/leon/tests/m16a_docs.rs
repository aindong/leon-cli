//! M16a criterion 14 (.agent/plans/2026-11-15-m16a-remote-ssh.md): remote use is documented
//! where people look (the flag and command reference is checked by m7b_cli_doc).

mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap()
}

#[test]
fn remote_use_is_documented() {
    let readme = doc("README.md");
    assert!(
        readme.lines().any(|l| l.starts_with('#') && l.to_lowercase().contains("work on your devbox")),
        "README has a \"Work on your devbox\" section"
    );
    assert!(readme.contains("leon hosts add"));
    let cli = doc("docs/cli.md");
    for s in ["`leon hosts add", "`leon hosts check", "`leon server proxy`", "-H, --host"] {
        assert!(cli.contains(s), "docs/cli.md: {s}");
    }
    let arch =
        // the hosts decisions moved to docs/architecture/hosts.md (file-size limit)
        [
            "docs/architecture.md",
            "docs/architecture/protocol-and-clients.md",
            "docs/architecture/state-and-hosts.md",
            "docs/architecture/hosts.md",
        ]
        .map(doc)
            .concat();
    for s in ["leon server proxy", "session.resume", "LEON_SSH", "reconnect", "hosts.json"] {
        assert!(arch.contains(s), "docs/architecture.md: {s}");
    }
}
