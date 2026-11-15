//! Daily-driver F6 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 4): the README's first
//! commands, run verbatim against a real server, read back `42` every time: the sequence waits for
//! Python's prompt before it types, so `send` can't land before Python starts.

mod help_walk;
mod support;

use std::process::Command;
use support::Env;

fn first_commands() -> String {
    let readme = std::fs::read_to_string(help_walk::repo().join("README.md")).unwrap();
    let section = readme.split("## The first five commands").nth(1).expect("the README has the section");
    help_walk::blocks(section, "sh").into_iter().next().expect("an sh block in the section")
}

#[test]
fn the_readmes_first_commands_read_back_42() {
    if !Command::new("python3").args(["-c", "1"]).status().is_ok_and(|s| s.success()) {
        eprintln!("skipped: no python3");
        return;
    }
    let block = first_commands();
    for run in 1..=3 {
        let env = Env::new();
        let bin = env.dir.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_leon"), bin.join("leon")).unwrap();
        // python3 starts a beat late, as on a cold or busy machine: the race the README must not have
        let python =
            String::from_utf8(Command::new("sh").args(["-c", "command -v python3"]).output().unwrap().stdout).unwrap();
        std::fs::write(bin.join("python3"), format!("#!/bin/sh\nsleep 0.3\nexec '{}' \"$@\"\n", python.trim()))
            .unwrap();
        std::fs::set_permissions(bin.join("python3"), std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
        // env.cmd()'s private LEON_* dirs and SHELL, on a clean env
        let mut sh = Command::new("/bin/sh");
        sh.args(["-c", &block]).current_dir(env.dir.path());
        sh.env_clear();
        for (k, v) in env.cmd().get_envs() {
            if let Some(v) = v {
                sh.env(k, v);
            }
        }
        let o = sh.env("PATH", &path).env("HOME", env.dir.path()).output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout).into_owned();
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success(), "run {run}: the block failed: {err}\n{out}");
        let lines: Vec<&str> = out.lines().collect();
        let typed =
            lines.iter().position(|l| l.contains("print(6 * 7)")).unwrap_or_else(|| panic!("run {run}:\n{out}"));
        let banner = lines.iter().position(|l| l.starts_with("Python 3") || l.contains("Type \"help\""));
        assert!(banner.is_some_and(|b| b < typed), "run {run}: the line was typed before Python started:\n{out}");
        assert!(lines[typed].starts_with(">>> "), "run {run}: typed at the prompt:\n{out}");
        assert_eq!(lines.get(typed + 1).map(|l| l.trim()), Some("42"), "run {run}:\n{out}");
        let _ = env.cmd().env("PATH", &path).args(["server", "stop"]).output();
    }
}
