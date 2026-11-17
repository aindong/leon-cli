//! M13 criterion 12 (with M12): the docs describe the switcher, Activity, notifications,
//! `leon notify`, the notify settings, the line cap and screen chunking, and the new method.

fn doc(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)).unwrap()
}

#[test]
fn the_docs_cover_m12_and_m13() {
    let readme = doc("docs/shortcuts.md");
    for row in ["| ⌘K |", "| ⌘⇧A |"] {
        assert!(readme.contains(row), "docs/shortcuts.md's table has {row}");
    }
    let cli = doc("docs/cli.md");
    assert!(cli.contains("### `leon notify"), "cli.md documents leon notify");
    let guide = doc("crates/leon/src/skill/guide.md");
    assert!(guide.contains("leon notify"), "the agent guide tells agents about leon notify");
    let lua = doc("docs/lua/settings.md");
    for s in ["notifications.blocked", "notifications.error", "notifications.done"] {
        assert!(lua.contains(s), "lua/settings.md documents {s}");
    }
    // main split the decisions into docs/architecture/ (M15)
    let arch = [
        "docs/architecture.md",
        "docs/architecture/protocol-and-clients.md",
        "docs/architecture/state-and-hosts.md",
        "docs/architecture/attention.md",
    ]
    .map(doc)
    .join("\n");
    for topic in
        ["session.switch", "client.sidebar.activity.toggle", "UNUserNotificationCenter", "16 MiB", "report_status"]
    {
        assert!(arch.contains(topic), "architecture.md covers {topic}");
    }
    let api = doc("docs/api.md");
    assert!(api.contains("dev.leon.terminal.report_status"), "api.md lists the new method");
    let contract = doc("crates/leon-server/src/contract.rs");
    assert!(contract.contains("\"dev.leon.terminal.report_status\""), "the contract names it");
}

#[test]
fn leon_skill_show_mentions_notify() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_leon")).args(["skill", "show"]).output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("leon notify"));
}
