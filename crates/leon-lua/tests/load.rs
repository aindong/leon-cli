//! M5a criteria 2, 6, 7 (.agent/plans/2026-11-14-m5a-lua-config.md): loading init.lua into a Config.

use leon_lua::{load, Config, Level, Target};
use serde_json::json;

fn load_src(src: &str) -> Result<Config, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    load(&path).map_err(|e| e.to_string())
}

fn errors(c: &Config) -> Vec<String> {
    c.diagnostics.iter().filter(|d| d.level == Level::Error).map(|d| d.to_string()).collect()
}

#[test]
fn bindings_modes_actions_and_lua_views() {
    let c = load_src(
        r#"
leon.bind("Cmd+T", "client.new_tab")
leon.bind("ctrl+b>c", "session.new_window", { label = "x", n = 2 })
leon.bind("ctrl+b>q", function(ctx, ev) return true end)
leon.mode("resize", { exclusive = true })
leon.bind("resize/left", "pane.resize", { dir = "left" })
leon.bind("scroll/k", "pane.scroll_up")
leon.action{ name = "say_hi", title = "Say hi", run = function(ctx, args) return args.who end,
  args = { who = "string", times = "integer?", extra = { type = "string", enum = { "a", "b" } } },
  keywords = { "hello" }, repeats = true }
leon.log("info", "loaded")
leon.log("plain")
leon.bind("cmd+shift+t", "client.tab.new")
leon.bind("ctrl+a>%", "pane.split", { direction = "right" })

local bs = leon.bindings()
assert(#bs == 7, "bindings: " .. #bs)
assert(bs[1].key == "cmd+t" and bs[1].mode == nil and bs[1].action == "client.new_tab")
assert(bs[2].args.label == "x" and bs[2].args.n == 2)
assert(type(bs[3].fn) == "function" and bs[3].action == nil)
assert(bs[4].mode == "resize" and bs[4].key == "left")
assert(bs[1].source:match("init.lua:2$"), bs[1].source)
assert(#leon.bindings({ mode = "resize" }) == 1)
assert(#leon.bindings({ key = "CTRL+B>C" }) == 1)
assert(#leon.bindings({ action = "pane.scroll_up" }) == 1)

local ms = leon.modes()
assert(#ms == 2, "modes: " .. #ms)
assert(ms[1].name == "resize" and ms[1].exclusive == true)
assert(ms[2].name == "scroll" and ms[2].exclusive == false)

local t = leon.json.decode('{"a":[1,2,3],"b":"c"}')
assert(t.a[3] == 3 and t.b == "c")
assert(leon.json.decode(leon.json.encode({ x = 1 })).x == 1)
assert(leon.json.encode({ x = 1 }, { indent = true }):find("\n"))
"#,
    )
    .unwrap();
    assert!(errors(&c).is_empty(), "{:?}", errors(&c));
    assert_eq!(c.bindings.len(), 7);
    assert_eq!(c.bindings[6].key.to_string(), "ctrl+a>%");
    let b = &c.bindings[1];
    assert_eq!(b.key.to_string(), "ctrl+b>c");
    assert_eq!(
        b.target,
        Target::Action { name: "session.new_window".into(), args: Some(json!({"label": "x", "n": 2})) }
    );
    assert!(b.source.ends_with("init.lua:3"), "{}", b.source);
    assert_eq!(c.bindings[2].target, Target::Function);
    assert_eq!(c.bindings[3].key.to_string(), "resize/left");

    assert_eq!(
        c.modes.iter().map(|m| (m.name.as_str(), m.exclusive)).collect::<Vec<_>>(),
        [("resize", true), ("scroll", false)]
    );

    assert_eq!(c.actions.len(), 1);
    let a = &c.actions[0];
    assert_eq!(
        (a.name.as_str(), a.title.as_str(), a.category.as_str(), a.repeats),
        ("say_hi", "Say hi", "Actions", true)
    );
    assert_eq!(a.keywords, ["hello"]);
    let schema = a.args.as_ref().unwrap();
    assert_eq!(schema["type"], "object");
    assert_eq!(schema["properties"]["who"], json!({"type": "string"}));
    assert_eq!(schema["properties"]["times"], json!({"type": "integer"}));
    assert_eq!(schema["properties"]["extra"]["enum"], json!(["a", "b"]));
    let mut req: Vec<_> = schema["required"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    req.sort();
    assert_eq!(req, ["extra", "who"]);

    assert_eq!(c.logs.len(), 2);
    assert_eq!(c.logs[0].level, Level::Info);
    assert_eq!(c.logs[0].message, "loaded");
    assert!(c.logs[0].at.ends_with("init.lua:11"), "{}", c.logs[0].at);
}

#[test]
fn prefix_conflicts_warn_and_newest_wins() {
    let c = load_src(
        r#"
leon.bind("ctrl+b", "a.one")
leon.bind("ctrl+b>c", "a.two")
leon.bind("ctrl+x>y>z", "a.three")
leon.bind("ctrl+x>y", "a.four")
leon.bind("ctrl+b>c", "a.five")
leon.bind("m/g", "a.six")
leon.bind("g>g", "a.seven")
"#,
    )
    .unwrap();
    let keys: Vec<_> = c.bindings.iter().map(|b| b.key.to_string()).collect();
    assert_eq!(keys, ["ctrl+b>c", "ctrl+x>y", "ctrl+b>c", "m/g", "g>g"]);
    let warns: Vec<_> = c.diagnostics.iter().filter(|d| d.level == Level::Warning).map(|d| d.to_string()).collect();
    assert_eq!(warns.len(), 2, "{warns:?}");
    assert!(
        warns[0].contains("init.lua:3") && warns[0].contains("ctrl+b>c") && warns[0].contains("ctrl+b"),
        "{}",
        warns[0]
    );
    assert!(warns[0].contains("init.lua:2"), "{}", warns[0]);
    assert!(warns[1].contains("ctrl+x>y>z") && warns[1].contains("init.lua:4"), "{}", warns[1]);
    assert!(errors(&c).is_empty());
}

#[test]
fn popen_and_execute_raise() {
    for call in ["io.popen('ls')", "os.execute('true')"] {
        let e = load_src(&format!("leon.bind('a', 'x.y')\n{call}\n")).unwrap_err();
        assert!(e.contains("leon.exec") && e.contains("init.lua:2"), "{call}: {e}");
    }
    // the rest of the stdlib is there
    load_src("assert(string.format('%d', 3) == '3'); assert(os.time() > 0); assert(io.write)").unwrap();
}

#[test]
fn missing_file_is_an_error() {
    let e = load(std::path::Path::new("/nonexistent/leon/init.lua")).unwrap_err().to_string();
    assert!(e.contains("/nonexistent/leon/init.lua"), "{e}");
}
