//! M17 criterion 14: `ssh host leon attach` (no -t) gives the client no terminal. It must say to
//! use `ssh -t` and exit, without writing escape sequences down the pipe.

mod support;

use std::process::Stdio;

use support::Env;

#[test]
fn without_a_terminal_it_says_use_ssh_t() {
    let env = Env::new();
    env.ok(&["run", "--", "sh"]);
    let out = env.cmd().arg("attach").stdin(Stdio::null()).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "no terminal: exit 1");
    assert!(out.stdout.is_empty(), "nothing on stdout: {:?}", String::from_utf8_lossy(&out.stdout));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ssh -t"), "it suggests ssh -t: {err}");
}
