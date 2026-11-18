//! Agent orchestration criterion 5: `leon skill setup` adds a status hook for each agent event
//! leon follows, beside the user's own hooks, twice without change; `remove` gives the file back
//! as it was; and the commands it wrote, run the way the agent runs them, report to the inbox.

mod support;

use serde_json::{json, Value};
use support::orch::{private, until, Orch};

/// (event, matcher) leon hooks per agent, beside SessionStart.
const CLAUDE: &[(&str, Option<&str>)] = &[
    ("SessionStart", None),
    ("UserPromptSubmit", None),
    ("PermissionRequest", None),
    ("PreToolUse", Some("AskUserQuestion")),
    ("PostToolUse", None),
    ("PostToolUseFailure", None),
    ("Stop", None),
];
const CODEX: &[(&str, Option<&str>)] = &[
    ("SessionStart", None),
    ("UserPromptSubmit", None),
    ("PermissionRequest", None),
    ("PostToolUse", None),
    ("Stop", None),
    ("Interrupt", None),
];

/// The user's own settings: hooks on events leon uses too, and other keys.
fn mine() -> Value {
    json!({
        "model": "x",
        "hooks": {
            "Stop": [{ "hooks": [{ "type": "command", "command": "say done" }] }],
            "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "~/bin/guard.sh" }] }],
            "SessionStart": [{ "hooks": [{ "type": "command", "command": "~/bin/ctx.sh" }] }]
        },
        "theme": "dark"
    })
}

/// leon's command for `event` (and `matcher`) in `v`, which must be the last group of that event.
fn ours(v: &Value, event: &str, matcher: Option<&str>) -> String {
    let groups = v["hooks"][event].as_array().unwrap_or_else(|| panic!("no {event} in {v}"));
    let g = groups.last().unwrap();
    assert_eq!(g["matcher"].as_str(), matcher, "{event}: {g}");
    let cmd = g["hooks"][0]["command"].as_str().unwrap().to_string();
    assert!(cmd.contains("leon skill hook"), "{event}: {cmd}");
    cmd
}

#[test]
fn setup_twice_then_remove_keeps_the_users_hooks() {
    let o = Orch::new();
    let (claude, codex) = (o.home.join(".claude/settings.json"), o.home.join(".codex/hooks.json"));
    for f in [&claude, &codex] {
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, serde_json::to_string_pretty(&mine()).unwrap()).unwrap();
    }
    let leon = |args: &[&str]| {
        let out = o.cmd().args(args).output().unwrap();
        assert!(out.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    leon(&["skill", "setup"]);
    let once = (std::fs::read_to_string(&claude).unwrap(), std::fs::read_to_string(&codex).unwrap());
    let out = leon(&["skill", "setup"]);
    assert_eq!((std::fs::read_to_string(&claude).unwrap(), std::fs::read_to_string(&codex).unwrap()), once);
    assert!(!out.contains("update"), "the second setup changes nothing: {out}");
    let status: Value = serde_json::from_str(&leon(&["skill", "status", "--json"])).unwrap();
    for a in status.as_array().unwrap() {
        assert_eq!(a["status"], "current", "{status}");
    }

    for (file, events) in [(&claude, CLAUDE), (&codex, CODEX)] {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
        for (event, matcher) in events {
            ours(&v, event, *matcher);
        }
        let first = |e: &str| v["hooks"][e][0].clone();
        assert_eq!(
            (first("Stop"), first("PreToolUse")),
            (mine()["hooks"]["Stop"][0].clone(), mine()["hooks"]["PreToolUse"][0].clone()),
            "the user's hooks come first, as they were"
        );
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "theme"], "keys stay in order");
    }
    assert!(!std::fs::read_to_string(&claude).unwrap().contains("Interrupt"), "Claude Code has no Interrupt event");

    leon(&["skill", "remove"]);
    for f in [&claude, &codex] {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        assert_eq!(v.to_string(), mine().to_string(), "remove gives the file back: {}", f.display());
    }
}

/// A fake agent that runs each installed command through `sh -c`, as the agent does, with `payloads`.
fn agent_running(o: &Orch, name: &str, file: &std::path::Path, payloads: &[(&str, Option<&str>, Value)]) -> String {
    let v: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let mut body = String::from("run() { printf '%s' \"$2\" | sh -c \"$1\"; }\n");
    for (event, matcher, payload) in payloads {
        let cmd = ours(&v, event, *matcher);
        let payload = payload.to_string();
        assert!(!cmd.contains('\'') && !payload.contains('\''), "quotable");
        body.push_str(&format!("run '{cmd}' '{payload}'\n"));
    }
    body.push_str("echo hooks-done\nexec sleep 300");
    o.script(name, &body).to_str().unwrap().to_string()
}

#[test]
fn the_installed_commands_report_to_the_inbox() {
    let o = Orch::new();
    let out = o.cmd().args(["skill", "setup", "--agent", "claude"]).output().unwrap();
    assert!(out.status.success());
    let out = private(o.env.cmd(), &o.home).args(["skill", "setup", "--agent", "codex"]).output().unwrap();
    assert!(out.status.success());
    let (s, boss) = o.env.session();

    let claude = agent_running(
        &o,
        "claude-fake",
        &o.home.join(".claude/settings.json"),
        &[
            ("UserPromptSubmit", None, json!({ "hook_event_name": "UserPromptSubmit", "prompt": "go" })),
            (
                "PermissionRequest",
                None,
                json!({ "hook_event_name": "PermissionRequest", "tool_name": "Bash",
            "tool_input": { "command": "make clean", "description": "Clean" } }),
            ),
            ("PostToolUse", None, json!({ "hook_event_name": "PostToolUse", "tool_name": "Bash" })),
            (
                "PreToolUse",
                Some("AskUserQuestion"),
                json!({ "hook_event_name": "PreToolUse", "tool_name": "AskUserQuestion",
            "tool_input": { "questions": [{ "question": "Tabs or spaces?" }] } }),
            ),
            (
                "PostToolUseFailure",
                None,
                json!({ "hook_event_name": "PostToolUseFailure", "tool_name": "AskUserQuestion" }),
            ),
            ("Stop", None, json!({ "hook_event_name": "Stop", "last_assistant_message": "Spaces it is." })),
        ],
    );
    let codex = agent_running(
        &o,
        "codex-fake",
        &o.home.join(".codex/hooks.json"),
        &[
            (
                "UserPromptSubmit",
                None,
                json!({ "hook_event_name": "UserPromptSubmit", "turn_id": "t1", "prompt": "go" }),
            ),
            (
                "PermissionRequest",
                None,
                json!({ "hook_event_name": "PermissionRequest", "turn_id": "t1",
            "tool_name": "shell", "tool_input": { "command": ["cargo", "publish"], "description": "Publish" } }),
            ),
            ("PostToolUse", None, json!({ "hook_event_name": "PostToolUse", "turn_id": "t1", "tool_name": "shell" })),
            (
                "Stop",
                None,
                json!({ "hook_event_name": "Stop", "turn_id": "t1", "stop_hook_active": false,
            "last_assistant_message": "Published." }),
            ),
            ("Interrupt", None, json!({ "hook_event_name": "Interrupt", "turn_id": "t2" })),
        ],
    );
    let wc = o.ok_inside(&s, &boss, &["agent", "start", "--label", "c", &claude, "t"]);
    let wx = o.ok_inside(&s, &boss, &["agent", "start", "--label", "x", &codex, "t"]);
    for w in [&wc, &wx] {
        o.env.screen_until(w, "the hooks", |t| t.contains("hooks-done"));
    }
    let inbox = until("five reports", 10, || {
        let v = o.json_inside(&s, &boss, &["inbox", "--json", "--peek"]);
        (v.as_array().unwrap().len() >= 5).then_some(v)
    });
    let of = |w: &str| -> Vec<(String, String, String)> {
        inbox
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["block_id"] == w)
            .map(|r| {
                let s = |k: &str| r[k].as_str().unwrap_or("").to_string();
                (s("state"), s("kind"), s("body"))
            })
            .collect()
    };
    let c = of(&wc);
    assert_eq!(c.len(), 3, "{inbox}");
    assert_eq!((c[0].0.as_str(), c[0].1.as_str()), ("blocked", "permission"));
    assert!(c[0].2.contains("make clean"), "{c:?}");
    assert_eq!((c[1].0.as_str(), c[1].1.as_str(), c[1].2.as_str()), ("blocked", "question", "Tabs or spaces?"));
    assert_eq!((c[2].0.as_str(), c[2].2.as_str()), ("done", "Spaces it is."));
    let x = of(&wx);
    assert_eq!(x.len(), 2, "{inbox}");
    assert!(x[0].2.contains("cargo publish"), "{x:?}");
    assert_eq!((x[1].0.as_str(), x[1].2.as_str()), ("done", "Published."));
    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    let state =
        |w: &str| team["workers"].as_array().unwrap().iter().find(|t| t["block_id"] == w).unwrap()["state"].clone();
    assert_eq!(state(&wx), "idle", "Codex's Interrupt: {team}");
}
