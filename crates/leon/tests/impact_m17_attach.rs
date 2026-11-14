//! M17 impact (loop-impact): the facts `leon attach` is safe because of, outside its own code.
//! 1. A server it autostarts inherits neither its blocked signals nor the watchdog's pipe: the
//!    server still stops on SIGTERM, and a SIGKILLed client is still restored.
//! 2. The size rule: it never fights another client that resized a pane; it refits only when its
//!    own layout changes.

mod support;

use serde_json::Value;
use support::host::{has_words, Host};
use support::Env;

#[test]
fn an_autostarted_server_inherits_no_mask_and_no_pipe() {
    let env = Env::new();
    let script = "\"$LEON\" attach; i=0; while stty -a | grep -q -- -icanon && [ $i -lt 100 ]; do sleep 0.05; \
                  i=$((i+1)); done; echo \"stty: $(stty -a | tr ' ' '\\n' | grep -E '^-?icanon$')\"; exec sleep 300";
    let h = Host::spawn(&env, script, &[]);
    h.wait_text("Pick a session");
    let status: Value = env.json(&["--autostart=false", "server", "status", "--json"]);
    let server = status["pid"].as_i64().unwrap_or_else(|| panic!("server status has a pid: {status}")) as i32;
    let out = std::process::Command::new("pgrep").args(["-P", &h.pid().to_string()]).output().unwrap();
    let attach: i32 = String::from_utf8_lossy(&out.stdout).lines().next().unwrap().trim().parse().unwrap();
    // SAFETY: signals to processes this test started
    unsafe { libc::kill(attach, libc::SIGKILL) };
    let s = h.wait_text("stty:");
    assert!(s.contains("stty: icanon"), "the watchdog restored the host, so the server holds no pipe:\n{s}");
    unsafe { libc::kill(server, libc::SIGTERM) };
    Host::until("the server stops on SIGTERM", || env.code(&["--autostart=false", "server", "status"]) == 4);
}

#[test]
fn another_clients_resize_is_left_alone() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh"]);
    let session = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    let size = || {
        let v = env.json(&["-b", &block, "block", "call", "size"]);
        (v["rows"].as_u64().unwrap(), v["cols"].as_u64().unwrap())
    };
    Host::until("the TUI fitted the pane", || size() == (23, 80));
    env.ok(&["-b", &block, "block", "call", "resize", r#"{"rows":10,"cols":30}"#]);
    // the pane's echo makes the TUI draw again after the resize; it must not refit
    h.send(b"echo mark-$((3*3))\r");
    h.wait_for("the echo drawn", |s| has_words(s, &["mark-9"]));
    assert_eq!(size(), (10, 30), "the TUI must not undo another client's resize");
    h.term.resize(30, 100).unwrap();
    Host::until("its own size change refits", || size() == (29, 100));
}
