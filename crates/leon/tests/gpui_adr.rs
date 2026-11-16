//! How leon uses GPUI (.agent/plans/2026-11-17-gpui-audit.md): the ADR exists where contributors
//! look, names the GPUI it was written for, and the rules a file walk can check hold.

mod help_walk;

use std::path::Path;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The app's sources under `crates/leon-app/src`, as (path from there, code without `//` lines).
fn app_sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(&p, root, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                let code: Vec<&str> = text.lines().filter(|l| !l.trim_start().starts_with("//")).collect();
                out.push((p.strip_prefix(root).unwrap().to_string_lossy().into_owned(), code.join("\n")));
            }
        }
    }
    let root = help_walk::repo().join("crates/leon-app/src");
    let mut files = Vec::new();
    walk(&root, &root, &mut files);
    files.sort();
    assert!(files.len() > 40, "found the app's sources: {}", files.len());
    files
}

/// The `[patch…]` sections of the workspace's Cargo.toml.
fn cargo_patches() -> String {
    let cargo = doc("Cargo.toml");
    cargo.split("\n[").filter(|t| t.starts_with("patch")).collect::<Vec<_>>().join("\n[")
}

/// The text of the ADR's `## <name>` section, up to the next `## `.
fn section<'a>(text: &'a str, name: &str) -> &'a str {
    let head = format!("\n## {name}\n");
    let at = text.find(&head).unwrap_or_else(|| panic!("docs/architecture/gpui.md has `## {name}`")) + head.len();
    let rest = &text[at..];
    &rest[..rest.find("\n## ").unwrap_or(rest.len())]
}

#[test]
fn the_adr_is_where_contributors_look() {
    let adr = doc("docs/architecture/gpui.md");
    assert!(adr.len() < 20_000, "docs/architecture/gpui.md stays under 20 KB: {}", adr.len());
    for s in ["Context", "Decision", "Rules", "Allowed outside GPUI", "Alternatives considered", "Consequences"] {
        section(&adr, s);
    }
    // each rule is a `- **R<n>.**` bullet, its lines up to the next bullet
    let rules: Vec<&str> = section(&adr, "Rules").split("\n- **R").skip(1).collect();
    assert!(rules.len() >= 8, "the ADR has its rules as `- **R<n>.**` bullets: {}", rules.len());
    for r in &rules {
        assert!(r.contains("Why:"), "each rule gives its reason: R{r}");
        assert!(r.contains("Check:"), "each rule says how it is checked: R{r}");
    }
    assert!(doc("docs/architecture.md").contains("(architecture/gpui.md)"), "docs/architecture.md links the ADR");
    let claude = doc("CLAUDE.md");
    assert!(
        claude.lines().any(|l| l.contains("docs/architecture/gpui.md") && l.contains("GPUI source")),
        "CLAUDE.md sends GPUI work to the ADR and GPUI's source"
    );
}

#[test]
fn the_adr_names_the_gpui_leon_builds_with() {
    // the lock, not Cargo.toml's caret requirement: what `cargo update` would really build
    let lock = doc("Cargo.lock");
    let entry = lock.split("[[package]]").find(|p| p.contains("\nname = \"gpui\"\n")).expect("Cargo.lock has gpui");
    let field = |k: &str| {
        let line = entry.lines().find(|l| l.starts_with(&format!("{k} = \""))).unwrap_or_else(|| panic!("gpui's {k}"));
        line.split('"').nth(1).unwrap().to_string()
    };
    let (version, source) = (field("version"), field("source"));
    let built = doc("docs/architecture/gpui.md")
        .lines()
        .find(|l| l.starts_with("Built against: "))
        .expect("docs/architecture/gpui.md has a `Built against: ` line")
        .to_string();
    let patched = cargo_patches();
    assert!(!patched.contains("gpui"), "R11: GPUI is used as published, no [patch] for it: {patched}");
    assert!(built.contains(&format!("GPUI {version}")), "a GPUI change (to {version}) amends the ADR first: {built}");
    if source.starts_with("registry+") {
        assert!(built.contains("crates.io"), "the ADR says the GPUI is crates.io's: {built}");
    } else {
        let rev = source.rsplit('#').next().unwrap();
        assert!(built.contains(rev), "a GPUI from {source} is named by its commit in the ADR: {built}");
    }
}

#[test]
fn raw_appkit_is_only_where_the_adr_allows_it() {
    let adr = doc("docs/architecture/gpui.md");
    let allowed = section(&adr, "Allowed outside GPUI");
    let rule = |n: &str| section(&adr, "Rules").lines().find(|l| l.starts_with(&format!("- **{n}."))).unwrap_or("");
    assert!(rule("R2").contains("AppKit"), "R2 is the AppKit rule the messages below cite");
    assert!(rule("R8").contains("thread"), "R8 is the threads rule the messages below cite");
    let mut users = 0;
    for (name, code) in app_sources() {
        if code.contains("msg_send!") || code.contains("objc2::") {
            users += 1;
            assert!(allowed.contains(&format!("`{name}`")), "{name} calls AppKit: list it under Allowed outside GPUI");
        }
        for poke in ["setNeedsDisplay", "displayLayer", "raw_window_handle", "HasWindowHandle"] {
            assert!(!code.contains(poke), "{name} uses {poke}: never schedule GPUI's frames through AppKit (R2)");
        }
    }
    assert!(users > 0, "the walk sees the AppKit calls there are");
}

#[test]
fn gpui_code_neither_spawns_threads_nor_blocks() {
    let mut views = 0;
    for (name, code) in app_sources() {
        // the view's submodules see GPUI through `use super::*`
        if code.contains("gpui") || name.starts_with("view") || name.starts_with("settings_window") {
            views += 1;
            for bad in ["thread::spawn", "thread::Builder", "thread::sleep", "block_on("] {
                assert!(
                    !code.contains(bad),
                    "{name} uses {bad}: GPUI code runs off-thread work on GPUI's executors (R8)"
                );
            }
        }
    }
    assert!(views > 20, "the walk sees the GPUI code: {views} files");
}
