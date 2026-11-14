//! M18 impact: `leon api list` now lists block methods, which the server doesn't answer as
//! methods of their own. Describing or calling one points at `leon block call` instead of a bare
//! "no method".

mod support;

use support::Env;

#[test]
fn block_methods_point_at_block_call() {
    let env = Env::new();
    let out = env.ok(&["api", "describe", "dev.leon.terminal.size"]);
    assert!(out.contains("leon block call dev.leon.terminal.size"), "{out}");
    let call = env.run(&["api", "call", "dev.leon.terminal.size"]);
    assert_eq!(call.status.code(), Some(1));
    let err = String::from_utf8_lossy(&call.stderr);
    assert!(err.contains("is a block method") && err.contains("leon block call"), "{err}");
    // Server methods still go through.
    assert!(env.json(&["api", "call", "server.version"])["api_version"].is_string());
}
