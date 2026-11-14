//! M5a review advisories: shebang line, strict boolean options, diagnostics through pcall.

use leon_lua::{load, Level};

fn load_src(src: &str) -> leon_lua::Config {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    load(&path).unwrap_or_else(|e| panic!("{e}"))
}

fn errors(c: &leon_lua::Config) -> Vec<String> {
    c.diagnostics.iter().filter(|d| d.level == Level::Error).map(|d| d.to_string()).collect()
}

#[test]
fn a_hash_first_line_is_ignored_and_lines_still_count() {
    let c = load_src("#!/usr/bin/env lua\nleon.bind('hyper+a', 'x.y')\n");
    assert!(errors(&c)[0].contains("init.lua:2:"), "{:?}", errors(&c));
}

#[test]
fn boolean_options_reject_non_booleans() {
    let c = load_src("leon.mode('m', { exclusive = 'false' })\nleon.mode('n', { exclusive = true })\n");
    assert_eq!(c.modes.len(), 1);
    assert!(errors(&c)[0].contains("exclusive must be true or false"), "{:?}", errors(&c));
}

#[test]
fn diagnostics_through_pcall_point_at_the_config_line() {
    let c = load_src("\npcall(leon.bind, 'hyper+a', 'x.y')\n");
    assert!(errors(&c)[0].contains("init.lua:2:"), "{:?}", errors(&c));
}
