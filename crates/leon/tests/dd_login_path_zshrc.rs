//! F1 as a zsh user meets it: tools put on PATH in ~/.zshrc (nvm, Claude Code's ~/.local/bin)
//! must be found by a server started with launchd's environment, as they are in the user's tab.

// macOS only: zsh is the default login shell there and Leon.app-from-Finder is the F1 case; Linux images may lack /bin/zsh
#![cfg(target_os = "macos")]

mod support;

use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use support::Env;

#[test]
fn a_tool_on_the_zshrc_path_is_found_by_a_finder_started_server() {
    let env = Env::new();
    let home = env.dir.path().join("home");
    let tools = home.join("tools");
    std::fs::create_dir_all(&tools).unwrap();
    // where nvm and most installers put PATH additions for zsh users
    std::fs::write(home.join(".zshrc"), "export PATH=\"$HOME/tools:$PATH\"\n").unwrap();
    let tool = tools.join("dd-zshrc-tool");
    std::fs::write(&tool, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();

    let launchd = || {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env_clear()
            .env("HOME", &home)
            .env("SHELL", "/bin/zsh")
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("LEON_RUNTIME_DIR", env.dir.path())
            .env("LEON_STATE_DIR", env.state_dir())
            .env("LEON_CONFIG", env.dir.path().join("no-init.lua"));
        c
    };
    let started = launchd().args(["server", "start"]).output().unwrap();
    assert!(started.status.success(), "{}", String::from_utf8_lossy(&started.stderr));
    let out = launchd().args(["run", "--wait", "--", "dd-zshrc-tool"]).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "zshrc tool not found by a launchd-started server: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
