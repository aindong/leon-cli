//! Found in M7b's security review: an empty server PATH must not leave an empty (current
//! directory) entry in a terminal's PATH.

mod support;

#[test]
fn an_empty_server_path_adds_no_cwd_entry() {
    let env = support::Env::new();
    let out = env.dir.path().join("path.txt");
    let script = format!("printf '%s' \"$PATH\" > '{}'", out.display());
    let o = env.cmd().env("PATH", "").args(["run", "--wait", "--", "/bin/sh", "-c", &script]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let path = std::fs::read_to_string(&out).unwrap();
    assert!(path.split(':').all(|e| !e.is_empty()), "PATH has an empty (cwd) entry: {path:?}");
}
