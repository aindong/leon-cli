//! The permission guard tells people from programs (docs/agents.md, "Only the user answers
//! permission prompts"): what lives on of a closed pane is a program, however short the pane's
//! life or whatever group its job moves to; a person's move of a pane is not a focus move; a prompt
//! raised where a program put the keys doesn't keep them; and what a program's key runs (a Lua
//! binding, or the key itself after one) can't type into a held prompt.

mod support;

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::orch::{Orch, READER};

fn view(o: &Orch, s: &str) -> Value {
    o.env.json(&["api", "call", "session.view", &json!({ "session_id": s }).to_string()])
}

fn keys_go_to(v: &Value) -> String {
    let w = v["windows"].as_array().unwrap().iter().find(|w| w["active"] == true).unwrap();
    w["zoomed_block_id"].as_str().or(w["focused_block_id"].as_str()).unwrap_or("").to_string()
}

fn focus(o: &Orch, s: &str, b: &str) {
    o.env.ok(&["api", "call", "session.focus_block", &json!({ "session_id": s, "block_id": b }).to_string()]);
}

/// A worker reading lines, held for a permission prompt; returns (session, worker).
fn held_reader(o: &Orch) -> (String, String) {
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", READER]);
    o.env.screen_until(&w, "the worker", |t| t.contains("ready>"));
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    (o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string(), w)
}

/// Waits (no requests meanwhile) for a job to write its `leon send`'s exit status to `code`.
fn tried(code: &std::path::Path) -> String {
    let end = Instant::now() + Duration::from_secs(20);
    while !code.exists() {
        assert!(Instant::now() < end, "the job never tried");
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_millis(100)); // the exit code's write lands
    std::fs::read_to_string(code).unwrap().trim().to_string()
}

/// Runs `calls` from a program in a session of its own; returns their stderr.
fn inside(o: &Orch, calls: &[(&str, Value)]) -> String {
    let out = o.env.dir.path().join("tries");
    let body: String = calls
        .iter()
        .enumerate()
        .map(|(i, (m, p))| format!("leon api call {m} '{p}' > /dev/null 2> '{}.{i}.err'\n", out.display()))
        .collect();
    let agent = o.script("agent", &format!("{body}echo tried; exec sleep 300"));
    let own = o.env.ok(&["new", "agents"]);
    let tab = o.env.ok(&["run", "-s", &own, "--", agent.to_str().unwrap()]);
    o.env.screen_until(&tab, "the agent's tries", |t| t.contains("tried"));
    (0..calls.len())
        .map(|i| std::fs::read_to_string(format!("{}.{i}.err", out.display())).unwrap_or_default())
        .collect()
}

#[test]
fn a_job_from_a_pane_no_guard_look_saw_still_counts_as_inside() {
    let o = Orch::new();
    let (_s, w) = held_reader(&o);
    let code = o.env.dir.path().join("code");
    // waits for the pane's program (its parent) to exit and the pane to close, then tries the prompt
    let job = o.script(
        "job",
        &format!(
            "trap '' HUP\nwhile kill -0 $PPID 2>/dev/null; do sleep 0.05; done\nsleep 2\nleon send -b {w} yes\necho $? > '{}'",
            code.display()
        ),
    );
    // its own process group, off the PTY; the pane's program exits at once, so no request sees it live
    let pane = o.script("flash", &format!("set -m\n'{}' </dev/null >/dev/null 2>&1 &\nexit 0", job.display()));
    o.env.ok(&["run", "--label", "flash", "--", pane.to_str().unwrap()]);
    assert_ne!(tried(&code), "0", "UNSEEN_PANE: a job from a pane no guard look saw typed into the held prompt");
}

#[test]
fn a_job_that_regroups_after_its_pane_closed_still_counts_as_inside() {
    let o = Orch::new();
    let (_s, w) = held_reader(&o);
    let dir = o.env.dir.path();
    let (go, code) = (dir.join("go"), dir.join("code"));
    let second = o.script(
        "second",
        &format!(
            "trap '' HUP\nwhile kill -0 $PPID 2>/dev/null; do sleep 0.05; done\nsleep 0.5\nleon send -b {w} yes\necho $? > '{}'",
            code.display()
        ),
    );
    // once the guard remembered its group: a new group in the same session (no setsid), then exit
    let first = o.script(
        "first",
        &format!(
            "trap '' HUP\nwhile [ ! -e '{}' ]; do sleep 0.1; done\nperl -e 'setpgrp(0, 0); exec @ARGV' '{}' &\nexit 0",
            go.display(),
            second.display()
        ),
    );
    let pane = o.script(
        "outliver",
        &format!("set -m\n'{}' </dev/null >/dev/null 2>&1 &\necho started; exec sleep 300", first.display()),
    );
    let p = o.env.ok(&["run", "--label", "outliver", "--", pane.to_str().unwrap()]);
    o.env.screen_until(&p, "the job", |t| t.contains("started"));
    o.env.ok(&["block", "close", &p]);
    o.env.ok(&["ls"]); // a guard look after the close
    std::fs::write(&go, "").unwrap();
    assert_ne!(tried(&code), "0", "REGROUPED: a job that changed its process group typed into the held prompt");
}

#[test]
fn a_persons_move_of_the_focused_block_away_does_not_hand_the_keys_to_a_held_prompt() {
    let o = Orch::new();
    let (s, w) = held_reader(&o);
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    let x = o.env.ok(&["run", "-s", &s, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    let other = o.env.ok(&["new", "elsewhere"]);
    let anchor = view(&o, &other)["windows"][0]["focused_block_id"].as_str().unwrap().to_string();
    // the person (this test runs outside every leon terminal) pulls F beside their pane elsewhere
    let p = json!({ "session_id": other, "block_id": f, "anchor_block_id": anchor });
    o.env.ok(&["api", "call", "session.move_block", &p.to_string()]);
    let v = view(&o, &s);
    assert_ne!(keys_go_to(&v), w, "PERSON_MOVE_RELAY: a person's move of F away left the keys on the held prompt: {v}");
    assert_eq!(keys_go_to(&v), x, "{v}");
}

#[test]
fn a_prompt_raised_where_a_program_put_the_keys_does_not_keep_them() {
    let o = Orch::new();
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", "echo ready; exec sleep 300"]);
    o.env.screen_until(&w, "the worker", |t| t.contains("ready"));
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    let err = inside(&o, &[("session.focus_block", json!({ "session_id": s, "block_id": w }))]);
    assert!(err.is_empty(), "W isn't held yet, so the move goes through: {err}");
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    // the report reaches the block's status a moment later
    let end = Instant::now() + Duration::from_secs(5);
    let mut v = view(&o, &s);
    while keys_go_to(&v) == w && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
        v = view(&o, &s);
    }
    assert_ne!(keys_go_to(&v), w, "HOLD_AFTER_FOCUS: a program put the keys on W, W then held, keys stayed: {v}");
    assert_eq!(keys_go_to(&v), f, "{v}");
}

#[test]
fn a_binding_run_by_a_programs_key_cannot_type_into_a_held_prompt() {
    let o = Orch::new();
    let (s, w) = held_reader(&o);
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    let binding = format!(
        "leon.bind(\"ctrl+g\", function(ctx)\n  leon.call(\"block.method\", {{ session_id = \"{s}\", block_id = \"{w}\", method = \"dev.leon.terminal.write\", args = {{ data = \"yes\\r\" }} }})\n  return true\nend)\n"
    );
    std::fs::write(o.env.dir.path().join("no-init.lua"), binding).unwrap();
    o.env.ok(&["api", "call", "server.config_reload", "{}"]);
    let err = inside(&o, &[("client.key", json!({ "session_id": s, "key": "ctrl+g" }))]);
    std::thread::sleep(Duration::from_millis(300)); // the binding's write lands, if it got through
    assert_eq!(
        o.count(&w, "GOT:"),
        0,
        "LUA_WRITE: a program's key ran a binding that typed into the prompt ({err}):\n{}",
        o.env.capture(&w)
    );
}

#[test]
fn a_key_from_inside_types_only_where_it_was_checked() {
    let o = Orch::new();
    let (s, w) = held_reader(&o);
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    let gate = o.env.dir.path().join("gate");
    // a binding that waits for the test, then lets the key through to the focused pane
    std::fs::write(
        o.env.dir.path().join("no-init.lua"),
        format!(
            "leon.bind(\"enter\", function(ctx)\n  for _ = 1, 100 do\n    local f = io.open(\"{}\")\n    if f then f:close() break end\n    pcall(leon.call, \"block.wait\", {{ block_id = \"{f}\", timeout_ms = 50 }})\n  end\n  return false\nend)\n",
            gate.display()
        ),
    )
    .unwrap();
    o.env.ok(&["api", "call", "server.config_reload", "{}"]);
    let p = json!({ "session_id": s, "key": "enter" });
    let agent = o.script(
        "agent",
        &format!("echo started\nleon api call client.key '{p}' >/dev/null 2>&1\necho tried\nexec sleep 300"),
    );
    let own = o.env.ok(&["new", "agents"]);
    let tab = o.env.ok(&["run", "-s", &own, "--", agent.to_str().unwrap()]);
    o.env.screen_until(&tab, "the key sent", |t| t.contains("started"));
    std::thread::sleep(Duration::from_millis(500)); // checked against F; the binding now waits
                                                    // the person goes to read the prompt, then lets the binding finish
    focus(&o, &s, &w);
    std::fs::write(&gate, "").unwrap();
    o.env.screen_until(&tab, "the key done", |t| t.contains("tried"));
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        o.count(&w, "GOT:"),
        0,
        "KEY_RETARGET: a program's key typed into the held prompt:\n{}",
        o.env.capture(&w)
    );
}

#[test]
fn a_closed_panes_program_that_ignores_the_hang_up_counts_as_inside() {
    let o = Orch::new();
    let (_s, w) = held_reader(&o);
    let dir = o.env.dir.path();
    let (go, code) = (dir.join("go"), dir.join("code"));
    // the pane's own program ignores the hang-up: it lives through the close's grace before the kill
    let pane = o.script(
        "stubborn",
        &format!(
            "trap '' HUP\necho started\nwhile [ ! -e '{}' ]; do sleep 0.05; done\nleon send -b {w} yes >/dev/null 2>&1\necho $? > '{}'\nexec sleep 300",
            go.display(),
            code.display()
        ),
    );
    let p = o.env.ok(&["run", "--label", "stubborn", "--", pane.to_str().unwrap()]);
    o.env.screen_until(&p, "the program", |t| t.contains("started"));
    o.env.ok(&["block", "close", &p]);
    std::fs::write(&go, "").unwrap();
    assert_ne!(tried(&code), "0", "HUP_IGNORED: a closed pane's program typed into the held prompt");
}

#[test]
fn a_binding_run_by_a_programs_key_cannot_exec_leon_into_a_held_prompt() {
    let o = Orch::new();
    let (s, w) = held_reader(&o);
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    // what Lua runs is a child of the server, which has the test's private LEON_* dirs
    let binding = format!(
        "leon.bind(\"ctrl+g\", function(ctx)\n  local r = leon.exec({{ \"{leon}\", \"send\", \"-b\", \"{w}\", \"yes\" }})\n  local f = io.open(\"{out}\", \"a\")\n  f:write(tostring(r.code) .. \" \" .. tostring(r.stdout) .. \" \" .. tostring(r.stderr) .. \"\\n\")\n  f:close()\n  return true\nend)\n",
        leon = env!("CARGO_BIN_EXE_leon"),
        out = o.env.dir.path().join("exec.out").display(),
    );
    std::fs::write(o.env.dir.path().join("no-init.lua"), binding).unwrap();
    o.env.ok(&["api", "call", "server.config_reload", "{}"]);
    let err = inside(&o, &[("client.key", json!({ "session_id": s, "key": "ctrl+g" }))]);
    // the exec's exit status and output, as the binding wrote them
    let got = std::fs::read_to_string(o.env.dir.path().join("exec.out")).unwrap_or_default();
    assert!(
        !got.starts_with("0 "),
        "LUA_EXEC: a program's key ran a binding whose leon.exec typed into the prompt ({err}): {got}"
    );
}

#[test]
fn a_close_moves_the_keys_to_a_free_pane_in_another_tab_not_onto_a_held_prompt() {
    let o = Orch::new();
    let x = o.env.ok(&["run", "--label", "x", "--", "sh", "-c", "exec sleep 300"]);
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = o.env.ok(&["split", "-b", &x, "--", "sh", "-c", "exec sleep 300"]);
    // another tab: a free pane F2, and a held H2 that has that tab's focus
    let f2 = o.env.ok(&["run", "-s", &s, "--", "sh", "-c", "exec sleep 300"]);
    let h2 = o.env.ok(&["split", "-b", &f2, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &h2);
    o.env.ok(&["notify", "-b", &h2, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    focus(&o, &s, &x);
    o.env.ok(&["notify", "-b", &h, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    // the person closes X: H beside it is held, and so is the other tab's focus
    o.env.ok(&["block", "close", &x]);
    let end = Instant::now() + Duration::from_secs(5);
    let mut v = view(&o, &s);
    while keys_go_to(&v) == h && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
        v = view(&o, &s);
    }
    assert_ne!(keys_go_to(&v), h, "KEYS_ON_HELD: F2 was free, yet the keys stayed on held H: {v}");
    assert_eq!(keys_go_to(&v), f2, "{v}");
}

#[test]
fn an_orphaned_job_of_a_closed_pane_whose_program_lives_on_counts_as_inside() {
    let o = Orch::new();
    let (_s, w) = held_reader(&o);
    let dir = o.env.dir.path();
    let (go, code) = (dir.join("go"), dir.join("code"));
    let job = o.script(
        "orphan",
        &format!(
            "trap '' HUP\nwhile [ ! -e '{}' ]; do sleep 0.05; done\nleon send -b {w} yes >/dev/null 2>&1\necho $? > '{}'",
            go.display(),
            code.display()
        ),
    );
    // the pane's program ignores the hang-up and lives on after the close; its job is handed to
    // pid 1 at once (a subshell that exits), in a process group of its own
    let pane = o.script(
        "leader",
        &format!(
            "trap '' HUP\n( set -m; '{}' </dev/null >/dev/null 2>&1 & )\necho started\nexec sleep 300",
            job.display()
        ),
    );
    let p = o.env.ok(&["run", "--label", "leader", "--", pane.to_str().unwrap()]);
    o.env.screen_until(&p, "the program", |t| t.contains("started"));
    o.env.ok(&["block", "close", &p]);
    std::fs::write(&go, "").unwrap();
    assert_ne!(tried(&code), "0", "ORPHAN_OF_LIVE_LEADER: a closed pane's orphaned job typed into the held prompt");
}

#[test]
fn a_close_moves_the_keys_to_a_free_float_rather_than_a_held_prompt() {
    let o = Orch::new();
    let x = o.env.ok(&["run", "--label", "x", "--", "sh", "-c", "exec sleep 300"]);
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let h = o.env.ok(&["split", "-b", &x, "--", "sh", "-c", "exec sleep 300"]);
    let f = o.env.ok(&["split", "-b", &x, "--", "sh", "-c", "exec sleep 300"]);
    o.env.ok(&["api", "call", "session.float_block", &json!({ "session_id": s, "block_id": f }).to_string()]);
    focus(&o, &s, &x);
    o.env.ok(&["notify", "-b", &h, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    // the person closes X: the only pane left in the tab that isn't held is the float
    o.env.ok(&["block", "close", &x]);
    let end = Instant::now() + Duration::from_secs(5);
    let mut v = view(&o, &s);
    while keys_go_to(&v) == h && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
        v = view(&o, &s);
    }
    assert_ne!(keys_go_to(&v), h, "FLOAT_SKIPPED: a free float was left, yet the keys stayed on held H: {v}");
    assert_eq!(keys_go_to(&v), f, "{v}");
}
