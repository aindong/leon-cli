//! Impact (M14): the restore waits for the start-up config to read its `resume` setting, and the
//! server serves nothing until it has restored. A slow init.lua must not hold every client (the
//! app's first screen) for its whole load: the wait is bounded, then the restore offers nothing.

mod support;

use std::time::{Duration, Instant};

use support::Env;

#[test]
fn a_slow_config_doesnt_hold_the_start() {
    let env = Env::new();
    let cfg = env.dir.path().join("init.lua");
    // About 4 s of work at load, within the 5 s load limit; the setting comes last.
    let lua = "local n = 0\nlocal t = os.time()\nwhile os.time() - t < 4 do n = n + 1 end\nleon.set(\"resume\", \"always\")\n";
    std::fs::write(&cfg, lua).unwrap();
    let t = Instant::now();
    let o = env.cmd().env("LEON_CONFIG", &cfg).args(["server", "start"]).output().unwrap();
    assert!(o.status.success(), "server start: {}", String::from_utf8_lossy(&o.stderr));
    env.ok(&["ls"]);
    assert!(t.elapsed() < Duration::from_millis(2500), "the first call waited {:?} for the config", t.elapsed());
}
