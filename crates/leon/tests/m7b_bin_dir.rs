//! Found in M7b's impact check: a terminal gets LEON_BIN_DIR (the leon binary's directory) at the
//! front of PATH, so `leon` resolves inside the app even when the user's PATH
//! lacks ~/.local/bin.

mod support;

#[test]
fn terminals_find_leon_through_leon_bin_dir() {
    let env = support::Env::new();
    let out = env.dir.path().join("out.txt");
    let script = format!(
        "{{ echo \"dir=$LEON_BIN_DIR\"; echo \"path=$PATH\"; echo \"found=$(command -v leon)\"; }} > '{}'",
        out.display()
    );
    env.ok(&["run", "--wait", "--", "/bin/sh", "-c", &script]);
    let screen = std::fs::read_to_string(&out).unwrap();
    let dir = std::path::Path::new(env!("CARGO_BIN_EXE_leon")).parent().unwrap().display().to_string();
    assert!(screen.contains(&format!("dir={dir}")), "LEON_BIN_DIR is the binary's dir:\n{screen}");
    assert!(screen.contains(&format!("path={dir}:")), "LEON_BIN_DIR leads PATH:\n{screen}");
    assert!(screen.contains(&format!("found={dir}/leon")), "leon resolves to this binary:\n{screen}");
}
