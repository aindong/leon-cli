//! Review fix (M14, criterion 13 / "a shell whose rc waits for input"): a restored block's resume
//! command must not be typed into a key the shell's own rc file reads with canonical mode off
//! (oh-my-zsh's update question is `read -r -k 1`, compinit's `read -q`). The shell is in front and
//! ICANON is off then, so the offer also waits for zle's bracketed paste; the command is offered at
//! the real prompt once the question is over.

mod support;

use serde_json::json;
use support::Env;

#[test]
fn the_resume_command_isnt_typed_into_an_rc_read() {
    if !std::path::Path::new("/bin/zsh").exists() {
        eprintln!("no /bin/zsh; skipped");
        return;
    }
    let env = Env::new();
    let zdot = env.dir.path().join("zdot");
    std::fs::create_dir(&zdot).unwrap();
    let start = || {
        let o = env.cmd().env("SHELL", "/bin/zsh").env("ZDOTDIR", &zdot).args(["server", "start"]).output().unwrap();
        assert!(o.status.success(), "server start: {}", String::from_utf8_lossy(&o.stderr));
    };
    start();
    let b = env.ok(&["run", "--", "/bin/zsh"]);
    let set = json!({ "command": ["sh", "-c", "echo resumed-$((40+2))"] }).to_string();
    env.ok(&["-b", &b, "block", "call", "set_resume", &set]);
    env.ok(&["server", "stop"]);

    // The restored login zsh asks a one-key question first, and gives up after 3 s with no answer.
    let rc = "printf 'update? [Y/n] '\nread -r -k 1 -t 3 answer\necho \"rc-read-[$answer]\"\n";
    std::fs::write(zdot.join(".zshrc"), rc).unwrap();
    start();

    let is_answer = |l: &str| l.contains("rc-read-[");
    let s = env.screen_until(&b, "the rc's question to end", |s| s.lines().any(is_answer));
    let line = s.lines().find(|l| is_answer(l)).unwrap().trim();
    assert!(line == "update? [Y/n] rc-read-[]" || line == "rc-read-[]", "the rc's own read got the resume keys:\n{s}");
    let s = env.screen_until(&b, "the offer at the prompt", |s| s.contains("sh -c 'echo resumed-$((40+2))'"));
    assert!(!s.lines().any(|l| l.trim() == "resumed-42"), "offered, not run:\n{s}");
}
