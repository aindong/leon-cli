//! Review fix for M5b criterion 9: `os.execute` from a config action fails it, like io.popen and
//! os.exit do, and the server keeps serving.

mod support;

use support::Env;

#[test]
fn os_execute_in_an_action_fails_it() {
    let env = Env::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, "leon.action{ name = 'execute_it', title = 'X', run = function() os.execute('true') end }\n")
        .unwrap();
    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let o = env.run(&["do", "execute_it"]);
    let err = String::from_utf8_lossy(&o.stderr);
    assert_eq!(o.status.code(), Some(1), "{err}");
    assert!(err.contains("os.execute is not available"), "{err}");
    env.ok(&["ls"]);
}
