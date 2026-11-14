//! Review finding (M5a): `xpcall(f, debug.traceback)` must work in a config.

use leon_lua::load;

#[test]
fn debug_traceback_is_available() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("init.lua");
    std::fs::write(
        &p,
        "local ok, tb = xpcall(function() error('x') end, debug.traceback)\nassert(not ok and tb:find('stack traceback'))\nleon.bind('a', 'x.y')\n",
    )
    .unwrap();
    let c = load(&p).unwrap_or_else(|e| panic!("load failed: {e}"));
    assert_eq!(c.bindings.len(), 1);
}
