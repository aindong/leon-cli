//! M18 criteria 8–9 (.agent/plans/2026-11-15-m18-integrator-promise.md): the Python client in
//! docs/integrators.md runs against a real server and does what the guide says; the guide covers
//! what a client in any language needs.

mod help_walk;
mod support;

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use support::Env;

fn guide() -> String {
    std::fs::read_to_string(help_walk::repo().join("docs/integrators.md")).expect("docs/integrators.md exists")
}

/// The text of each fenced block tagged `lang`.
fn blocks(doc: &str, lang: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur: Option<(bool, String)> = None;
    for line in doc.lines() {
        if let Some(info) = line.trim_start().strip_prefix("```") {
            match cur.take() {
                Some((true, text)) => out.push(text),
                Some(_) => {}
                None => cur = Some((info.trim() == lang, String::new())),
            }
        } else if let Some((_, text)) = cur.as_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    out
}

/// Run the guide's client with `cmd`'s environment; return what it printed.
fn run_client(code: &str, dir: &std::path::Path, mut cmd: Command) -> String {
    let script = dir.join("client.py");
    std::fs::write(&script, code).unwrap();
    let mut child =
        cmd.arg("-I").arg(&script).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("python3 is installed");
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("the example client didn't finish in 30 s");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let (mut out, mut err) = (String::new(), String::new());
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    child.stderr.take().unwrap().read_to_string(&mut err).unwrap();
    assert!(status.success(), "the client failed ({status}):\n{out}\n{err}");
    out
}

/// python3 with the environment of `leon` (`env_of`): the same runtime and state dirs.
fn python(env_of: &Command) -> Command {
    let mut c = Command::new("python3");
    c.env_clear();
    for (k, v) in env_of.get_envs() {
        match v {
            Some(v) => c.env(k, v),
            None => c.env_remove(k),
        };
    }
    c.env("PATH", std::env::var_os("PATH").unwrap_or_default());
    c
}

fn check(out: &str, api_version: &str) {
    assert!(out.contains(&format!("api_version {api_version}")), "{out}");
    assert!(out.contains("already-here"), "it lists the sessions: {out}");
    assert!(out.contains("exit 3"), "it ran `exit 3` and waited for it: {out}");
}

#[test]
fn python_client_runs_against_a_real_server() {
    let code = blocks(&guide(), "python");
    assert_eq!(code.len(), 1, "the guide has exactly one python block");
    let lines = code[0].lines().count();
    assert!((30..=70).contains(&lines), "the example is about 50 lines, not {lines}");

    // With LEON_RUNTIME_DIR set.
    let env = Env::new();
    env.ok(&["server", "start"]);
    env.ok(&["new", "already-here"]);
    let api = env.json(&["api", "call", "server.version"]);
    let api_version = api["api_version"].as_str().unwrap();
    check(&run_client(&code[0], env.dir.path(), python(&env.cmd())), api_version);

    // The default path: no LEON_RUNTIME_DIR or XDG_RUNTIME_DIR, under a private HOME.
    let home = Env::new();
    let leon = |args: &[&str]| {
        let mut c = home.cmd();
        c.env_remove("LEON_RUNTIME_DIR").env_remove("XDG_RUNTIME_DIR").env("HOME", home.dir.path());
        c.args(args);
        c
    };
    let out = leon(&["new", "already-here"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(home.dir.path().join(".local/state/leon/run/server.sock").exists(), "the server took the default path");
    let printed = run_client(&code[0], home.dir.path(), python(&leon(&[])));
    let _ = leon(&["--autostart=false", "server", "stop"]).output();
    check(&printed, api_version);
}

#[test]
fn guide_covers_what_a_client_needs() {
    let g = guide();
    for needle in [
        "server.sock",
        "LEON_RUNTIME_DIR",
        "NDJSON",
        "\"error\"",
        "\"seq\"",
        "session.attach_all",
        "session.resume",
        "block.watch",
        "leon server proxy",
        "## Compatibility",
        "deprecated",
        "api_version",
        "api-snapshot.json",
        "minor",
        "major",
        "optional",
        "replacement",
        "removal",
    ] {
        assert!(g.contains(needle), "docs/integrators.md mentions {needle}");
    }
}
