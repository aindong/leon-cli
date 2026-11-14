//! Impact proof for M5a: a `leon.*` call made from a metamethod while another `leon.*` call reads
//! its arguments must not hit the load state's borrow twice (that would panic).

use leon_lua::{load, Level};

#[test]
fn leon_calls_from_metamethods_do_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        r#"
local n = 0
local sneaky = function(name)
  return setmetatable({}, { __index = function(_, k)
    n = n + 1
    leon.bind("f" .. n, "a.from_" .. name)
    leon.bindings(); leon.modes()
    if k == "run" then return function() end end
    if k == "name" then return "meta" end
    if k == "title" then return "Meta" end
    return nil
  end })
end
leon.mode("m", sneaky("mode"))
leon.action(sneaky("action"))
leon.bindings(sneaky("bindings"))
leon.unbind(sneaky("unbind"))
leon.bind("x", "a.args", sneaky("args"))
leon.json.encode(sneaky("encode"), sneaky("opts"))
"#,
    )
    .unwrap();
    let c = load(&path).unwrap_or_else(|e| panic!("{e}"));
    assert!(c.bindings.len() > 5, "{:?}", c.bindings);
    assert!(c.actions.iter().any(|a| a.name == "meta"));
    // unbind's table has neither id nor mode, so that one call is a diagnosed error, not a panic
    assert!(c.diagnostics.iter().any(|d| d.level == Level::Error && d.message.contains("leon.unbind")));
}
