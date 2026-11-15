//! Daily-driver criterion 2 (.agent/plans/2026-11-15-daily-driver-fixes.md): a program that isn't
//! on PATH fails with a short message naming it, not one that prints the whole PATH.

mod support;

use support::Env;

#[test]
fn a_missing_program_is_named_in_a_short_message() {
    let env = Env::new();
    let path = std::env::var("PATH").unwrap();
    let out = env.run(&["run", "--", "dd-no-such-program"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("dd-no-such-program"), "names the program: {err}");
    assert!(err.len() < 300, "short ({} bytes): {err}", err.len());
    assert!(!err.contains(&path) && !err.contains(":/usr/bin"), "no PATH dump: {err}");
}
