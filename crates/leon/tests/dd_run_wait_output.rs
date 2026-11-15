//! Daily-driver F5 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 2): `leon run --wait`
//! prints the program's text when stdout isn't a terminal (or with `--output`), so an agent gets
//! the output and the status in one call, and the block still goes away unless kept open.

mod support;

use support::host::Host;
use support::Env;

const PROGRAM: [&str; 3] = ["sh", "-c", "echo why-$((6*7)); exit 7"];

fn run_wait(env: &Env, flags: &[&str]) -> (i32, Vec<String>) {
    let o = env.cmd().args(["run", "--wait"]).args(flags).arg("--").args(PROGRAM).output().unwrap();
    let out = String::from_utf8(o.stdout).unwrap();
    (o.status.code().unwrap(), out.lines().map(str::to_string).collect())
}

fn blocks(env: &Env) -> String {
    env.ok(&["block", "ls"])
}

#[test]
fn piped_run_wait_prints_the_output_and_closes_the_block() {
    let env = Env::new();
    env.ok(&["new", "w"]);
    let (code, lines) = run_wait(&env, &["-s", "w"]);
    assert_eq!(code, 7, "{lines:?}");
    assert!(lines[0].starts_with("block:"), "the block ID comes first: {lines:?}");
    assert!(lines[1..].iter().any(|l| l == "why-42"), "then the program's text: {lines:?}");
    assert!(!blocks(&env).contains(&lines[0]), "the block closed: {}", blocks(&env));
    assert_eq!(env.code(&["wait", &lines[0]]), 7, "wait still knows its status");

    let (code, lines) = run_wait(&env, &["-s", "w", "--output=false"]);
    assert_eq!(code, 7);
    assert_eq!(lines.len(), 1, "--output=false prints only the ID: {lines:?}");

    let (code, lines) = run_wait(&env, &["-s", "w", "--keep-open"]);
    assert_eq!(code, 7);
    assert!(lines[1..].iter().any(|l| l == "why-42"), "{lines:?}");
    assert!(blocks(&env).contains(&lines[0]), "--keep-open keeps it: {}", blocks(&env));
}

#[test]
fn on_a_terminal_it_prints_only_with_output() {
    let env = Env::new();
    env.ok(&["new", "w"]);
    let script = "\"$LEON\" -s w run --wait -- sh -c 'echo tty-$((6*7))'; echo plain-rc=$?; \
                  \"$LEON\" -s w run --wait --output -- sh -c 'echo forced-$((6*7))'; echo forced-rc=$?; exec sleep 300";
    let h = Host::spawn(&env, script, &[]);
    let s = h.wait_text("forced-rc=0");
    assert!(s.contains("plain-rc=0"), "{s}");
    assert!(!s.contains("tty-42"), "a terminal gets no copy by default:\n{s}");
    assert!(s.contains("forced-42"), "--output prints it on a terminal too:\n{s}");
}

#[test]
fn a_reader_that_stops_early_still_gets_the_status_and_the_block_closes() {
    // `leon run --wait -- … | head -1`: the pipe is closed before the text is written
    use std::io::BufRead;
    let env = Env::new();
    env.ok(&["new", "w"]);
    let mut child = env
        .cmd()
        .args(["-s", "w", "run", "--wait", "--", "sh", "-c", "sleep 0.5; seq 1 3000; exit 7"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut id = String::new();
    std::io::BufReader::new(child.stdout.take().unwrap()).read_line(&mut id).unwrap(); // then dropped
    assert_eq!(child.wait().unwrap().code(), Some(7), "the program's status, not a panic");
    assert!(!blocks(&env).contains(id.trim()), "the block closed: {}", blocks(&env));
}
