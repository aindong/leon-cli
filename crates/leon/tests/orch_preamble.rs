//! A worker whose agent has leon's hooks reports by its last message, so finishing or asking costs
//! the person no extra approval for a `leon notify` command; an agent without them is told to run
//! `leon notify`. One unread report reads "to read it".

mod support;

use std::path::Path;
use std::process::Command;

use support::orch::{until, Orch};

/// The prompt (last argument) a worker named `name` got, started with `extra` agent arguments in
/// `dir`, by a caller whose environment `env` adjusts.
fn prompt(o: &Orch, s: &str, boss: &str, name: &str, dir: &Path, extra: &[&str], env: impl Fn(&mut Command)) -> String {
    let out = o.env.dir.path().join(format!("prompt-{name}"));
    let agent = o.script(
        name,
        &format!("for a in \"$@\"; do last=$a; done; printf '%s' \"$last\" > '{}'; exec sleep 300", out.display()),
    );
    let _ = std::fs::remove_file(&out); // a worker of the same name ran before
    let mut c = o.inside(s, boss);
    env(&mut c);
    let mut args = vec!["agent", "start", "-c", dir.to_str().unwrap(), agent.to_str().unwrap(), "the task", "--"];
    args.extend(extra);
    let done = c.args(&args).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    until("the worker's prompt", 15, || std::fs::read_to_string(&out).ok().filter(|p| !p.is_empty()))
}

fn hooked(p: &str) -> bool {
    p.contains("last message") && !p.contains("leon notify")
}

#[test]
fn hooked_agents_report_by_their_last_message() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let project = o.env.dir.path().join("project");
    std::fs::create_dir_all(project.join(".claude")).unwrap();
    let keep = |_: &mut Command| {};
    let p = prompt(&o, &s, &boss, "claude", &project, &[], keep);
    assert!(p.contains("leon notify --state done"), "no hooks yet: report with leon notify: {p}");

    // leon's hooks in the user's settings, at their usual place
    let out = o.cmd().args(["skill", "setup", "--agent", "claude"]).output().unwrap();
    assert!(out.status.success());
    let usual = |c: &mut Command| {
        c.env_remove("CLAUDE_CONFIG_DIR");
    };
    assert!(hooked(&prompt(&o, &s, &boss, "claude", &project, &[], usual)), "user settings");
    // a config dir set for the caller may not be the pane's: not trusted
    assert!(!hooked(&prompt(&o, &s, &boss, "claude", &project, &[], keep)), "CLAUDE_CONFIG_DIR set");

    // in the project's settings.local.json, as a project-only install has them
    let settings = std::fs::read_to_string(o.home.join(".claude/settings.json")).unwrap();
    std::fs::write(project.join(".claude/settings.local.json"), &settings).unwrap();
    assert!(hooked(&prompt(&o, &s, &boss, "claude", &project, &[], keep)), "project settings");
    assert!(!hooked(&prompt(&o, &s, &boss, "claude", &project, &["-p"], keep)), "print mode runs no Stop hook");
    assert!(!hooked(&prompt(&o, &s, &boss, "codex", &project, &[], usual)), "codex: hooks need trusting first");
    assert!(!hooked(&prompt(&o, &s, &boss, "my-agent", &project, &[], usual)), "an agent leon has no hooks for");

    let mut off: serde_json::Value = serde_json::from_str(&settings).unwrap();
    off["disableAllHooks"] = serde_json::json!(true);
    std::fs::write(project.join(".claude/settings.local.json"), off.to_string()).unwrap();
    assert!(!hooked(&prompt(&o, &s, &boss, "claude", &project, &[], keep)), "hooks turned off");
}

#[test]
fn one_report_reads_it() {
    let o = Orch::new();
    let (s, boss) = o.reader("boss");
    o.env.ok(&["notify", "-b", &boss, "--state", "working"]);
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "t"]);
    o.env.ok(&["notify", "-b", &w, "--state", "done", "--body", "finished"]);
    o.env.ok(&["notify", "-b", &boss, "--state", "done"]);
    until("the notice", 15, || {
        (o.count(&boss, "GOT: You have 1 leon message. Run leon inbox to read it.") == 1).then_some(())
    });
}
