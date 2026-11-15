//! Update criterion 19 (.agent/plans/2026-11-16-self-update-core.md): an update does not touch a
//! running server. The server started from the old binary keeps its pid, its sessions and their
//! programs, takes new work, and the update says what to do for the new version to take effect.

mod release_kit;
mod update_kit;

use std::process::Command;

use update_kit::{run, Install, Out, Release, VERSION};

/// The server this test started, stopped when the test ends however it ends.
struct Started<'a>(&'a Install);

impl Started<'_> {
    /// The test binary as a client of the install's server: after the update the installed leon
    /// is the release's stand-in, which only knows `--version`.
    fn client(&self, args: &[&str]) -> Out {
        let mut c = Command::new(env!("CARGO_BIN_EXE_leon"));
        c.env("LEON_RUNTIME_DIR", self.0.home.join("run"))
            .env("LEON_STATE_DIR", self.0.home.join("state"))
            .env("LEON_CONFIG", self.0.home.join("no-init.lua"))
            .env("LEON_AUTOSTART", "0")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("LEON_SERVER");
        run(c.args(args), 60)
    }

    fn pid(&self) -> String {
        let o = self.client(&["server", "status"]);
        assert_eq!(o.code, 0, "the server runs: {}", o.text());
        o.out.split_whitespace().skip_while(|w| *w != "pid").nth(1).expect("server status names the pid").to_string()
    }
}

impl Drop for Started<'_> {
    fn drop(&mut self) {
        let _ = self.client(&["server", "stop", "--discard"]);
    }
}

/// `nap`: how long the session's program sleeps, different for each test so that neither finds
/// the other's.
fn a_running_server_survives(leon: Install, nap: &str) {
    let rel = Release::new();
    let srv = rel.serve();
    // the installed leon starts its own server: that server runs the binary the update replaces
    let o = run(
        leon.cmd(&rel, &srv).env("LEON_AUTOSTART", "1").env("SHELL", "/bin/sh").args(["run", "--", "sleep", nap]),
        60,
    );
    assert_eq!(o.code, 0, "a session with a running program: {}", o.text());
    let block = o.out.trim().to_string();
    let server = Started(&leon);
    let pid = server.pid();
    let exe = release_kit::sh(&leon.home, &format!("ps -o command= -p {pid}"));
    assert!(exe.contains(leon.exe.to_str().unwrap()), "the server runs the installed binary: {exe}");

    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"), "the binary on disk is the new one");
    assert!(o.out.contains("leon server stop"), "it says how the server gets the new version: {}", o.text());
    assert!(o.out.contains("keeps") && o.out.contains("session"), "and that it keeps running until then: {}", o.text());

    assert_eq!(server.pid(), pid, "the same server is still running");
    let sessions: serde_json::Value = serde_json::from_str(&server.client(&["ls", "--json"]).out).unwrap();
    assert_eq!(sessions.as_array().unwrap().len(), 1, "the session is still there: {sessions}");
    let o = server.client(&["block", "inspect", &block, "--json"]);
    assert_eq!(o.code, 0, "its block is still there: {}", o.text());
    let running = release_kit::sh(&leon.home, "ps ax -o command=");
    assert!(running.lines().any(|l| l.trim() == format!("sleep {nap}")), "and its program is still running");
    let o = server.client(&["run", "--wait", "--", "true"]);
    assert_eq!(o.code, 0, "the old server still starts programs: {}", o.text());
}

#[test]
fn a_running_server_survives_a_cli_update() {
    a_running_server_survives(Install::cli(), "31337");
}

// macOS only: the app kind is a zip of Leon.app, and only macOS has it
#[cfg(target_os = "macos")]
#[test]
fn a_server_running_from_inside_the_app_survives_the_swap() {
    a_running_server_survives(Install::app(), "31338");
}
