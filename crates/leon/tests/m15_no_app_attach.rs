//! M15 (.agent/plans/2026-11-15-m15-linux-server-cli.md, criterion 1 after `leon attach` landed):
//! a build without the app runs `leon attach` when `leon` alone is in a terminal; piped, it prints
//! the help (m15_no_app.rs).

#![cfg(not(all(feature = "app", target_os = "macos")))]

mod support;

use support::host::{assert_restored, attach_then_stty, Host};
use support::Env;

#[test]
fn leon_alone_in_a_terminal_attaches() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh", "-c", "echo hello-m15; exec sh"]);
    env.screen_until(&block, "the shell's first line", |s| s.contains("hello-m15"));
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    env.ok(&["session", "rename", &session, "m15-demo"]);

    let script = attach_then_stty("").replacen("\"$LEON\" attach ", "\"$LEON\" -s m15-demo", 1);
    let h = Host::spawn(&env, &script, &[]);
    h.wait_text("hello-m15");
    h.wait_for("the status bar names the session", |_| h.last_row().contains("m15-demo"));
    h.send(b"\x02d");
    assert_restored(&h, 0);
}

#[test]
fn leon_alone_with_h_attaches_to_that_host() {
    let r = support::remote::Remote::new();
    r.ok(&["-H", "ssh://box", "new", "far"]);
    let vars = r.vars();
    let vars: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let script = attach_then_stty("").replacen("\"$LEON\" attach ", "\"$LEON\" -H ssh://box", 1);
    let h = Host::spawn(&r.local, &script, &vars);
    // no target: attach's picker, listing the host's session
    h.wait_for("the host's sessions in the picker", |s| s.contains("Pick a session") && s.contains("far"));
    h.send(b"\x1b");
    assert_restored(&h, 0);
}
