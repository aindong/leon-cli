//! The permission guard's focus rule (docs/agents.md, "Only the user answers permission prompts"):
//! each way the review found to put the keys on a held prompt as a side effect, from a program
//! inside leon, leaves them elsewhere; and closing a pane that has no keys goes through.

mod support;

use serde_json::{json, Value};
use support::orch::Orch;

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

/// W held, F split beside it with the keys; returns (session, W, F).
fn setup(o: &Orch) -> (String, String, String) {
    let w = o.env.ok(&["run", "--label", "worker", "--", "sh", "-c", "echo ready; exec sleep 300"]);
    o.env.screen_until(&w, "the worker", |t| t.contains("ready"));
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    assert_eq!(keys_go_to(&view(o, &s)), f);
    (s, w, f)
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
fn dock_block_from_inside_cannot_focus_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f) = setup(&o);
    let err = inside(&o, &[("session.dock_block", json!({ "session_id": s, "block_id": w, "side": "bottom" }))]);
    assert_eq!(
        keys_go_to(&view(&o, &s)),
        f,
        "DOCK_RELAY: dock_block from inside put the keys on the held prompt ({err})"
    );
}

#[test]
fn unfloat_block_from_inside_cannot_focus_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f) = setup(&o);
    o.env.ok(&["api", "call", "session.float_block", &json!({ "session_id": s, "block_id": w }).to_string()]);
    focus(&o, &s, &f);
    assert_eq!(keys_go_to(&view(&o, &s)), f);
    let err = inside(&o, &[("session.unfloat_block", json!({ "session_id": s, "block_id": w }))]);
    assert_eq!(
        keys_go_to(&view(&o, &s)),
        f,
        "DOCK_RELAY: unfloat_block from inside put the keys on the held prompt ({err})"
    );
}

#[test]
fn moving_the_focused_block_to_another_session_does_not_focus_the_prompt() {
    let o = Orch::new();
    let (s, w, f) = setup(&o);
    let x = o.env.ok(&["run", "-s", &s, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    let other = o.env.ok(&["new", "elsewhere"]);
    let anchor = view(&o, &other)["windows"][0]["focused_block_id"].as_str().unwrap().to_string();
    let err =
        inside(&o, &[("session.move_block", json!({ "session_id": other, "block_id": f, "anchor_block_id": anchor }))]);
    let v = view(&o, &s);
    assert_ne!(
        keys_go_to(&v),
        w,
        "JOIN_RELAY: move_block of F into another session moved the keys onto the held prompt ({err}): {v}"
    );
    assert_eq!(keys_go_to(&v), x, "{v}");
}

#[test]
fn a_move_from_another_tab_does_not_unzoom_onto_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f) = setup(&o);
    let x = o.env.ok(&["split", "-b", &f, "--", "sh", "-c", "exec sleep 300"]);
    let t = o.env.ok(&["run", "-s", &s, "--", "sh", "-c", "exec sleep 300"]);
    let y = o.env.ok(&["split", "-b", &t, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &t);
    focus(&o, &s, &f);
    assert_eq!(keys_go_to(&view(&o, &s)), f);
    let err = inside(
        &o,
        &[
            ("session.zoom_block", json!({ "session_id": s, "block_id": x, "zoom": true })),
            ("session.swap_blocks", json!({ "session_id": s, "block_id": f, "other_block_id": w })),
            ("session.move_block", json!({ "session_id": s, "block_id": y, "anchor_block_id": x })),
        ],
    );
    let v = view(&o, &s);
    assert_ne!(
        keys_go_to(&v),
        w,
        "UNZOOM_RELAY: zoom, swap and a move from another tab put the keys on the held prompt ({err}): {v}"
    );
}

#[test]
fn ending_the_focused_program_does_not_hand_the_keys_to_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f) = setup(&o);
    // a pane that isn't held, in another tab: F's own tab has only the prompt left once it closes
    let x = o.env.ok(&["run", "-s", &s, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    let p = json!({ "session_id": s, "block_id": f, "method": "dev.leon.terminal.write", "args": { "data": "\u{3}" } });
    let err = inside(&o, &[("block.method", p)]);
    let t = std::time::Instant::now();
    let v = loop {
        let v = view(&o, &s);
        if !v.to_string().contains(&f) || t.elapsed().as_secs() > 10 {
            break v;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert!(!v.to_string().contains(&f), "F never closed: {v}");
    assert_ne!(
        keys_go_to(&v),
        w,
        "EXIT_RELAY: Ctrl-C into F from inside closed it and the keys went to the held prompt ({err}): {v}"
    );
    assert_eq!(keys_go_to(&v), x, "the keys go to the pane left that isn't held: {v}");
}

#[test]
fn a_key_binding_run_from_inside_cannot_steer_onto_a_held_prompt() {
    let o = Orch::new();
    let (s, _w, f) = setup(&o);
    std::fs::write(
        o.env.dir.path().join("no-init.lua"),
        "leon.bind(\"ctrl+g\", function(ctx)\n  leon.session.focus_next_pane{ wrap = true }\n  return true\nend)\n",
    )
    .unwrap();
    o.env.ok(&["api", "call", "server.config_reload", "{}"]);
    let err = inside(&o, &[("client.key", json!({ "session_id": s, "key": "ctrl+g" }))]);
    assert_eq!(
        keys_go_to(&view(&o, &s)),
        f,
        "KEY_RELAY: client.key from inside ran a binding that moved the keys onto the held prompt ({err})"
    );
}

#[test]
fn closing_a_background_worker_while_another_waits_goes_through() {
    let o = Orch::new();
    let home = o.env.ok(&["run", "--label", "home", "--", "sh", "-c", "echo ready; exec sleep 300"]);
    o.env.screen_until(&home, "home", |t| t.contains("ready"));
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let done = o.env.ok(&["run", "-s", &s, "--label", "done", "--", "sh", "-c", "echo ready; exec sleep 300"]);
    o.env.screen_until(&done, "done", |t| t.contains("ready"));
    let wait = o.env.ok(&["run", "-s", &s, "--label", "waiting", "--", "sh", "-c", "echo ready; exec sleep 300"]);
    o.env.screen_until(&wait, "waiting", |t| t.contains("ready"));
    o.env.ok(&["notify", "-b", &wait, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    focus(&o, &s, &home);
    assert_eq!(keys_go_to(&view(&o, &s)), home);
    let err = inside(&o, &[("block.close", json!({ "session_id": s, "block_id": done }))]);
    assert!(err.is_empty(), "refused: block.close of a background tab's finished worker: {err}");
    assert_eq!(keys_go_to(&view(&o, &s)), home);
}
