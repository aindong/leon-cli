//! M19 criterion 1 (.agent/plans/2026-11-15-m19-tmux-migrants.md): `leon.bind`'s 4th argument
//! `{ repeats = true }` (tmux `bind -r`) and `leon.set("repeat_time", ms)`.

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
fn repeats_rides_on_the_binding() {
    let c = load_src(
        r#"
leon.bind("ctrl+b>h", "pane.focus.left", nil, { repeats = true })
leon.bind("ctrl+b>x", "pane.close")
leon.bind("ctrl+b>l", "pane.focus.right", { n = 1 }, { repeats = false })
leon.bind("ctrl+b>k", function() return true end, nil, { repeats = true })
leon.bind("ctrl+b>j", "pane.focus.down", nil, { ["repeat"] = true })
leon.set("repeat_time", 300)
"#,
    );
    assert!(errors(&c).is_empty(), "{:?}", errors(&c));
    let rep: Vec<(String, bool)> = c.bindings.iter().map(|b| (b.key.sequence(), b.repeats)).collect();
    assert_eq!(
        rep,
        [("ctrl+b>h", true), ("ctrl+b>x", false), ("ctrl+b>l", false), ("ctrl+b>k", true), ("ctrl+b>j", true)]
            .map(|(k, r)| (k.to_string(), r))
    );
    assert_eq!(c.settings.repeat_time, 300);
    assert_eq!(load_src("").settings.repeat_time, 500, "unset: the default (500 ms)");
}

#[test]
fn bad_repeat_options_are_refused_with_a_reason() {
    let cases = [
        (r#"leon.bind("ctrl+b>h", "pane.focus.left", { repeats = true })"#, "4th"),
        (r#"leon.bind("ctrl+b>h", "pane.focus.left", nil, { repeats = "yes" })"#, "repeats"),
        (r#"leon.bind("ctrl+b>h", "pane.focus.left", nil, { again = true })"#, "again"),
        (r#"leon.bind("ctrl+b>h", "pane.focus.left", nil, 5)"#, "table"),
        (r#"leon.set("repeat_time", -1)"#, "repeat_time"),
        (r#"leon.set("repeat_time", 20000)"#, "repeat_time"),
        (r#"leon.set("repeat_time", 1.5)"#, "repeat_time"),
        (r#"leon.set("repeat_time", "500")"#, "repeat_time"),
    ];
    for (src, word) in cases {
        let c = load_src(src);
        let e = errors(&c);
        assert_eq!(e.len(), 1, "{src} should be refused once: {e:?}");
        assert!(e[0].contains(word), "{src}: {:?} should name {word}", e[0]);
        assert!(c.bindings.is_empty() && !c.setting_sources.contains_key("repeat_time"), "{src} was skipped");
    }
    assert_eq!(load_src(r#"leon.set("repeat_time", 0)"#).settings.repeat_time, 0, "0 turns repeat off");
}
