//! Themes criterion 8 (.agent/plans/2026-11-17-themes.md): `leon theme list` lists every built-in
//! theme, `leon actions` has the switcher, and the docs name the themes and the switcher's key.

mod support;

use support::Env;

const NAMES: [&str; 8] =
    ["leon-dark", "leon-light", "leon-maroon", "leon-tide", "leon-coal", "leon-linen", "leon-sand", "leon-mist"];

fn doc(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)).unwrap()
}

#[test]
fn theme_list_names_every_built_in_theme() {
    let env = Env::new();
    let conf = env.dir.path().join("conf/init.lua");
    let o = env.cmd().env("LEON_CONFIG", &conf).args(["theme", "list", "--json"]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&o.stdout).unwrap();
    let listed: Vec<&str> = rows.iter().filter_map(|r| r["name"].as_str()).collect();
    assert_eq!(listed, NAMES, "built-in first, in order");
    assert!(rows.iter().all(|r| r["builtin"] == true), "{rows:?}");
    let dark = rows.iter().filter(|r| r["dark"] == true).count();
    assert_eq!(dark, 4, "four dark, four light: {rows:?}");
}

#[test]
fn leon_actions_lists_the_switcher() {
    let env = Env::new();
    let text = env.ok(&["actions"]);
    assert!(text.lines().any(|l| l.starts_with("client.theme.change")), "{text}");
}

#[test]
fn the_docs_name_the_themes_and_the_switcher() {
    let cli = doc("docs/cli/settings.md");
    for name in NAMES {
        assert!(cli.contains(name), "docs/cli/settings.md names {name}");
    }
    let readme = doc("README.md");
    assert!(readme.contains("eight themes"), "the README's themes line");
    let shortcuts = doc("docs/shortcuts.md");
    let row = shortcuts.lines().find(|l| l.starts_with("| ⌘⇧T |")).expect("docs/shortcuts.md's table has ⌘⇧T");
    assert!(row.contains("theme"), "{row}");
    assert!(doc("docs/cli/settings.md").contains("Change Theme…"), "docs/cli/settings.md names the switcher");
}
