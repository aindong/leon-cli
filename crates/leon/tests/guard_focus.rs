//! The permission guard covers focus moves (docs/architecture/attention.md): a program inside a
//! leon terminal can't move the focus onto a block held for a permission prompt, where a person's
//! next keys would answer it, directly or by closing or zooming what has the focus; what keeps the
//! focus off the prompt goes through; and a person can still focus it.

mod support;

use serde_json::{json, Value};
use support::orch::Orch;

/// A worker that waits on a permission prompt (reported from outside, as its hook would).
fn held(o: &Orch, args: &[&str]) -> String {
    let w = o.env.ok(&[&["run"][..], args, &["--", "sh", "-c", "echo ready; exec sleep 300"]].concat());
    o.env.screen_until(&w, "the worker", |t| t.contains("ready"));
    o.env.ok(&["notify", "-b", &w, "--state", "blocked", "--kind", "permission", "--body", "Bash: rm -rf build"]);
    w
}

fn view(o: &Orch, s: &str) -> Value {
    o.env.json(&["api", "call", "session.view", &json!({ "session_id": s }).to_string()])
}

/// Where the session's keys go now: the active window's zoomed block, else its focused one.
fn keys_go_to(v: &Value) -> String {
    let w = v["windows"].as_array().unwrap().iter().find(|w| w["active"] == true).unwrap();
    w["zoomed_block_id"].as_str().or(w["focused_block_id"].as_str()).unwrap_or("").to_string()
}

/// Run `calls` (method, params) from a program in a pane of its own; each one's exit status and
/// whether it was refused for the prompt, and what it said.
fn from_inside(o: &Orch, s: &str, calls: &[(&str, Value)]) -> Vec<(String, bool, String)> {
    let out = o.env.dir.path().join("tries");
    let body: String = calls
        .iter()
        .enumerate()
        .map(|(i, (m, p))| {
            format!("leon api call {m} '{p}' > /dev/null 2> '{0}.{i}.err'; echo $? > '{0}.{i}.code'\n", out.display())
        })
        .collect();
    let agent = o.script("focus-agent", &format!("{body}echo tried; exec sleep 300"));
    // in a session of its own, so the target session's active tab and focus stay as they are
    let _ = s;
    let own = o.env.ok(&["new", "agents"]);
    let tab = o.env.ok(&["run", "-s", &own, "--label", "agent", "--", agent.to_str().unwrap()]);
    o.env.screen_until(&tab, "the agent's tries", |t| t.contains("tried"));
    (0..calls.len())
        .map(|i| {
            let code = std::fs::read_to_string(format!("{}.{i}.code", out.display())).unwrap().trim().to_string();
            let err = std::fs::read_to_string(format!("{}.{i}.err", out.display())).unwrap();
            (code, err.contains("permission prompt"), err)
        })
        .collect()
}

/// W (held) and F (focused) in the first tab, which is active; Z (held) alone in a second tab.
fn setup(o: &Orch) -> (String, String, String, String, String) {
    let w = held(o, &["--label", "worker"]);
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    let z = held(o, &["-s", &s, "--label", "other"]);
    let v = view(o, &s);
    let tab2 = v["windows"][1]["window_id"].as_str().unwrap().to_string();
    // a person puts the focus back on F in the first tab
    o.env.ok(&["api", "call", "session.focus_block", &json!({ "session_id": s, "block_id": f }).to_string()]);
    assert_eq!(keys_go_to(&view(o, &s)), f);
    (s, w, f, z, tab2)
}

#[test]
fn a_program_cannot_steer_the_focus_onto_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f, z, tab2) = setup(&o);
    let tab1 = view(&o, &s)["windows"][0]["window_id"].as_str().unwrap().to_string();
    let p = |extra: Value| {
        let mut v = json!({ "session_id": s });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        v
    };
    let refused = [
        // direct
        ("session.focus_block", p(json!({ "block_id": w }))),
        ("session.focus_block", p(json!({ "block_id": z }))),
        ("session.focus_window", p(json!({ "window_id": tab2 }))),
        ("session.zoom_block", p(json!({ "block_id": w }))),
        ("session.float_block", p(json!({ "block_id": w }))),
        // where it lands depends on the layout: refused while the active tab holds a prompt
        ("session.focus_next_pane", p(json!({ "wrap": true }))),
        ("session.focus_previous_pane", p(json!({ "wrap": true }))),
        ("session.focus_direction", p(json!({ "direction": "left", "wrap": true }))),
        // indirect: closing or moving what has the focus, or the active tab
        ("block.close", p(json!({ "block_id": f }))),
        ("session.close_window", p(json!({ "window_id": tab1 }))),
        ("session.move_block", p(json!({ "block_id": f, "anchor_block_id": z }))),
        ("session.toggle", p(json!({ "label": "worker" }))),
        // the focus stays with the place: swapping F with W puts the keys on W
        ("session.swap_blocks", p(json!({ "block_id": f, "other_block_id": w }))),
    ];
    let got = from_inside(&o, &s, &refused);
    for ((m, params), (code, why, err)) in refused.iter().zip(&got) {
        assert!(code != "0" && *why, "let through: {m} {params} (exit {code}): {err}");
    }
    let v = view(&o, &s);
    assert_eq!(keys_go_to(&v), f, "the keys still go to F: {v}");
    assert!(v["windows"].as_array().unwrap().iter().all(|w| w["zoomed_block_id"].is_null()), "{v}");
}

#[test]
fn what_keeps_the_focus_off_the_prompt_goes_through() {
    let o = Orch::new();
    let (s, w, f, _z, _tab2) = setup(&o);
    let p = |extra: Value| {
        let mut v = json!({ "session_id": s });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        v
    };
    let allowed = [
        ("session.zoom_block", p(json!({ "block_id": f }))),
        ("session.zoom_block", p(json!({ "block_id": f, "zoom": false }))),
        (
            "session.new_split",
            p(
                json!({ "anchor_id": f, "direction": "vertical", "side": "after", "layout": { "block": {} }, "focus": true }),
            ),
        ),
        ("session.new_window", p(json!({}))),
    ];
    let got = from_inside(&o, &s, &allowed);
    for ((m, params), (code, why, err)) in allowed.iter().zip(&got) {
        assert!(code == "0" && !*why, "refused: {m} {params} (exit {code}): {err}");
    }
    assert_ne!(keys_go_to(&view(&o, &s)), w);
}

#[test]
fn a_person_can_still_focus_the_prompt() {
    let o = Orch::new();
    let (s, w, _f, z, tab2) = setup(&o);
    // the test runs outside every leon terminal: a person, as the app and leon attach outside leon are
    o.env.ok(&["api", "call", "session.focus_block", &json!({ "session_id": s, "block_id": w }).to_string()]);
    assert_eq!(keys_go_to(&view(&o, &s)), w);
    o.env.ok(&["api", "call", "session.focus_window", &json!({ "session_id": s, "window_id": tab2 }).to_string()]);
    assert_eq!(keys_go_to(&view(&o, &s)), z);
}
