//! M5a criterion 3 (.agent/plans/2026-11-14-m5a-lua-config.md): the three `leon.unbind` forms.

use leon_lua::{load, Config, Level};

fn load_src(src: &str) -> Config {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    let c = load(&path).unwrap_or_else(|e| panic!("{e}"));
    let errs: Vec<_> = c.diagnostics.iter().filter(|d| d.level == Level::Error).map(|d| d.to_string()).collect();
    assert!(errs.is_empty(), "{errs:?}");
    c
}

fn keys(c: &Config) -> Vec<String> {
    c.bindings.iter().map(|b| b.key.to_string()).collect()
}

#[test]
fn unbind_key_removes_exact_matches_and_keeps_sequences() {
    let c = load_src(
        r#"
leon.bind("ctrl+b>c", "a.seq")
leon.bind("ctrl+t", "a.one")
leon.bind("Ctrl+T", "a.two")
leon.bind("ctrl+t", function() end)
assert(leon.unbind("ctrl+b") == 0)
assert(leon.unbind("CONTROL+t") == 3)
assert(leon.unbind("ctrl+t") == 0)
"#,
    );
    assert_eq!(keys(&c), ["ctrl+b>c"]);
    let unbinds: Vec<_> = c.unbinds.iter().map(|u| (u.key.to_string(), u.action.clone())).collect();
    assert_eq!(unbinds, [("ctrl+b".to_string(), None), ("ctrl+t".into(), None), ("ctrl+t".into(), None)]);
}

#[test]
fn unbind_key_with_target_filters_by_action_or_function() {
    let c = load_src(
        r#"
local f = function() end
local g = function() end
leon.bind("ctrl+t", "a.one")
leon.bind("ctrl+t", "a.two")
leon.bind("ctrl+t", f)
leon.bind("ctrl+t", g)
assert(leon.unbind("ctrl+t", "a.one") == 1)
assert(leon.unbind("ctrl+t", f) == 1)
assert(leon.unbind("ctrl+t", "nope") == 0)
local left = leon.bindings()
assert(#left == 2 and left[1].action == "a.two" and left[2].fn == g)
"#,
    );
    assert_eq!(c.bindings.len(), 2);
    let unbinds: Vec<_> = c.unbinds.iter().map(|u| (u.key.to_string(), u.action.clone())).collect();
    assert_eq!(unbinds, [("ctrl+t".to_string(), Some("a.one".to_string())), ("ctrl+t".into(), Some("nope".into()))]);
}

#[test]
fn unbind_mode_key_is_recorded_as_a_block() {
    let c = load_src(
        r#"
leon.bind("resize/left", "pane.resize")
leon.bind("left", "pane.focus")
assert(leon.unbind("resize/left") == 1)
assert(leon.unbind("resize/h") == 0)
"#,
    );
    assert_eq!(keys(&c), ["left"]);
    let unbinds: Vec<_> = c.unbinds.iter().map(|u| u.key.to_string()).collect();
    assert_eq!(unbinds, ["resize/left", "resize/h"]);
}

#[test]
fn unbind_mode_table_removes_the_mode_and_its_bindings() {
    let c = load_src(
        r#"
leon.mode("resize", { exclusive = true })
leon.bind("resize/left", "pane.resize")
leon.bind("resize/right", "pane.resize")
leon.bind("scroll/k", "pane.scroll")
leon.bind("left", "pane.focus")
assert(leon.unbind{ mode = "resize" } == 2)
assert(#leon.modes() == 1)
"#,
    );
    assert_eq!(keys(&c), ["scroll/k", "left"]);
    assert_eq!(c.modes.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["scroll"]);
}

#[test]
fn unbind_binding_removes_exactly_that_one() {
    let c = load_src(
        r#"
leon.bind("ctrl+t", "a.one")
leon.bind("ctrl+t", "a.two")
leon.bind("ctrl+u", "a.three")
local b = leon.bindings({ action = "a.two" })[1]
assert(leon.unbind(b) == 1)
assert(leon.unbind(b) == 0)
"#,
    );
    let actions: Vec<_> = c.bindings.iter().map(|b| format!("{:?}", b.target)).collect();
    assert_eq!(keys(&c), ["ctrl+t", "ctrl+u"]);
    assert!(actions[0].contains("a.one"), "{actions:?}");
}

#[test]
fn bad_unbind_calls_are_diagnosed_not_raised() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, "leon.bind('a', 'x.y')\nlocal n, e = leon.unbind('hyper+a')\nassert(n == nil and e)\nleon.unbind{}\n").unwrap();
    let c = load(&path).unwrap();
    let errs: Vec<_> = c.diagnostics.iter().filter(|d| d.level == Level::Error).map(|d| d.to_string()).collect();
    assert_eq!(errs.len(), 2, "{errs:?}");
    assert!(errs[0].contains("init.lua:2") && errs[0].contains("hyper"), "{}", errs[0]);
    assert!(errs[1].contains("init.lua:4"), "{}", errs[1]);
    assert_eq!(c.bindings.len(), 1);
}
