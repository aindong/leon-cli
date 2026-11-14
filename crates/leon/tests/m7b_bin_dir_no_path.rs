//! Found in M7b's review: a server started with no PATH still gives terminals the system PATH.

mod support;

#[test]
fn a_server_started_without_path_keeps_the_system_path_in_terminals() {
    let env = support::Env::new();
    let out = env.dir.path().join("out.txt");
    let script = format!("command -v ls > '{}'", out.display());
    // the CLI autostarts the server, which inherits this environment
    let o = env.cmd().env_remove("PATH").args(["run", "--wait", "--", "/bin/sh", "-c", &script]).output().unwrap();
    let found = std::fs::read_to_string(&out).unwrap_or_default();
    assert!(
        o.status.success() && found.trim().ends_with("/ls"),
        "ls not found in a terminal: status {:?}, {found:?}",
        o.status.code()
    );
}
