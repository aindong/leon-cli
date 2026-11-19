//! Cursor shaders are documented where users and contributors look: docs/cursor-shaders.md (how
//! to use it, every uniform leon declares, what works and what doesn't, the cost, and that leon
//! ships no shader), the settings table, and the ADR amendment for the overlay beside GPUI.

mod help_walk;

fn doc(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_page_says_how_what_and_what_not() {
    let page = doc("docs/cursor-shaders.md");
    for heading in ["## Use", "## The shader", "## What works and what doesn't", "## Cost"] {
        assert!(page.contains(&format!("\n{heading}\n")), "docs/cursor-shaders.md has {heading}");
    }
    for words in [
        "leon ships none",
        "Shadertoy-style",
        "macOS only",
        "device pixels",
        "CRT",
        "bloom",
        "leon config check",
        "Reduce Motion",
        "cursor_shader_duration",
    ] {
        assert!(page.contains(words), "docs/cursor-shaders.md says {words:?}");
    }
    // the other terminal whose shaders these are is never named in leon's docs
    for doc_path in ["docs/cursor-shaders.md", "docs/architecture/cursor-shaders.md", "docs/lua/settings.md"] {
        assert!(!doc(doc_path).to_lowercase().contains("ghostty"), "{doc_path} names no other terminal");
    }
    // the page's example is a config that loads
    let lua = help_walk::blocks(&page, "lua").into_iter().next().expect("a lua example");
    let dir = tempfile::tempdir().unwrap();
    let init = dir.path().join("init.lua");
    std::fs::write(&init, &lua).unwrap();
    let cfg = leon_lua::load(&init).unwrap();
    assert!(cfg.diagnostics.is_empty(), "{:?}", cfg.diagnostics);
    assert!(cfg.settings.cursor_shader.ends_with(".glsl"));
}

// macOS only: the wrapper's prefix is built on macOS only; the page lists each uniform it declares.
#[cfg(target_os = "macos")]
#[test]
fn every_uniform_leon_declares_is_on_the_page() {
    let page = doc("docs/cursor-shaders.md");
    let block = leon_app::cursor_shader::PREFIX.split("uniform Globals {").nth(1).unwrap().split("};").next().unwrap();
    let names: Vec<&str> = block
        .lines()
        .filter_map(|l| l.trim().strip_suffix(';')?.split_whitespace().last())
        .map(|n| n.split('[').next().unwrap())
        .collect();
    assert!(names.len() > 20, "{names:?}");
    for n in names.into_iter().chain(["iChannelTime", "iChannel0", "CURSORSTYLE_BAR"]) {
        assert!(page.contains(&format!("`{n}")), "docs/cursor-shaders.md lists {n}");
    }
}

#[test]
fn the_settings_and_the_adr_name_it() {
    let settings = doc("docs/lua/settings.md");
    for key in ["`cursor_shader`", "`cursor_shader_duration`"] {
        assert!(settings.lines().any(|l| l.starts_with(&format!("| {key} |"))), "docs/lua/settings.md lists {key}");
    }
    assert!(doc("docs/lua.md").contains("(cursor-shaders.md)"));
    let adr = doc("docs/architecture/cursor-shaders.md");
    assert!(adr.len() < 20_000, "under 20 KB: {}", adr.len());
    for words in ["## Measurement", "## How it goes away", "## Linux (future work, no code)", "31.9 ms", "R5"] {
        assert!(adr.contains(words), "the ADR has {words:?}");
    }
    assert!(doc("docs/architecture.md").contains("(architecture/cursor-shaders.md)"));
    let gpui = doc("docs/architecture/gpui.md");
    assert!(gpui.contains("- `cursor_shader/overlay.rs`:"), "gpui.md lists the overlay under Allowed outside GPUI");
    assert!(gpui.contains("[cursor-shaders.md](cursor-shaders.md)"));
}
