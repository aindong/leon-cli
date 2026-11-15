//! M19 criterion 10 (.agent/plans/2026-11-15-m19-tmux-migrants.md): `leon.set("hint_patterns",
//! {...})` adds hint patterns, checked when the config loads.

use leon_lua::{load, Config, Level};

fn load_src(src: &str) -> Config {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    load(&path).unwrap_or_else(|e| panic!("{e}"))
}

fn errors(c: &Config) -> Vec<String> {
    c.diagnostics.iter().filter(|d| d.level == Level::Error).map(|d| d.message.clone()).collect()
}

#[test]
fn good_patterns_load_in_order() {
    let c = load_src(r#"leon.set("hint_patterns", { "[A-Z]+-\\d+", "id=(\\w+)" })"#);
    assert!(errors(&c).is_empty(), "{:?}", errors(&c));
    assert_eq!(c.hint_patterns, vec![r"[A-Z]+-\d+".to_string(), r"id=(\w+)".to_string()]);
    assert!(load_src("").hint_patterns.is_empty());
}

#[test]
fn bad_patterns_are_refused_naming_the_problem() {
    let many = (0..33).map(|i| format!("\"x{i}\"")).collect::<Vec<_>>().join(", ");
    let long = "a".repeat(300);
    let cases = [
        (r#"leon.set("hint_patterns", { "(" })"#.to_string(), "("),
        (r#"leon.set("hint_patterns", "abc")"#.to_string(), "list"),
        (r#"leon.set("hint_patterns", { 5 })"#.to_string(), "string"),
        (r#"leon.set("hint_patterns", { "" })"#.to_string(), "empty"),
        (format!(r#"leon.set("hint_patterns", {{ {many} }})"#), "32"),
        (format!(r#"leon.set("hint_patterns", {{ "{long}" }})"#), "256"),
    ];
    for (src, word) in cases {
        let c = load_src(&src);
        let e = errors(&c);
        assert_eq!(e.len(), 1, "{src} should be refused once: {e:?}");
        assert!(e[0].contains(word), "{src}: {:?} should name {word}", e[0]);
        assert!(c.hint_patterns.is_empty(), "{src} set nothing");
    }
}
