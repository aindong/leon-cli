//! Review: a `leon server run` started in a terminal still reads the login shell's PATH; the
//! interactive probe must not get stopped by job control on that terminal.

mod support;

use std::os::unix::fs::PermissionsExt;

use support::host::Host;
use support::Env;

#[test]
fn a_server_run_in_a_terminal_reads_the_login_path() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    let tools = home.join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    // zsh -i takes the terminal (tcsetpgrp) and is stopped from a background group; bash where zsh isn't
    let (shell, rc) =
        if std::path::Path::new("/bin/zsh").exists() { ("/bin/zsh", ".zshrc") } else { ("/bin/bash", ".profile") };
    std::fs::write(home.join(rc), "PATH=\"$HOME/tools:$PATH\"; export PATH\n").unwrap();
    let tool = tools.join("dd-ctty-tool");
    std::fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
    let script = "\"$LEON\" server run >/dev/null 2>&1 & \
        until \"$LEON\" --autostart=false server status >/dev/null 2>&1; do sleep 0.1; done; \
        \"$LEON\" --autostart=false run --wait -- dd-ctty-tool; echo \"rc=$?\"; \
        \"$LEON\" --autostart=false server stop >/dev/null 2>&1; exec sleep 300";
    let home_s = home.to_str().unwrap();
    let h = Host::spawn(&env, script, &[("HOME", home_s), ("SHELL", shell), ("PATH", "/usr/bin:/bin")]);
    let s = h.wait_for("the run's exit code", |s| s.contains("rc="));
    assert!(s.contains("rc=0"), "login PATH not adopted under a controlling terminal:\n{s}");
}
