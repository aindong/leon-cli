//! The permission guard covers focus moves (docs/architecture/attention.md): the keys only land on
//! a block held for a permission prompt, where a person's next keys would answer it, when a person
//! moves them there. A program inside a leon terminal that asks for that gets `permission_pending`;
//! one that does it as a side effect (moving by direction, closing or moving what has the keys) has
//! the keys moved off again; and a person can still focus it, by any move.

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

/// W (held), F (focused) and X in the first tab, which is active; Z (held) alone in a second tab.
fn setup(o: &Orch) -> (String, String, String, String, String, String) {
    let w = held(o, &["--label", "worker"]);
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    // beside W, so the previous pane from X is W
    let x = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    let z = held(o, &["-s", &s, "--label", "other"]);
    let v = view(o, &s);
    let tab2 = v["windows"][1]["window_id"].as_str().unwrap().to_string();
    // a person puts the focus back on F in the first tab
    o.env.ok(&["api", "call", "session.focus_block", &json!({ "session_id": s, "block_id": f }).to_string()]);
    assert_eq!(keys_go_to(&view(o, &s)), f);
    (s, w, f, z, tab2, x)
}

fn focus(o: &Orch, s: &str, b: &str) {
    o.env.ok(&["api", "call", "session.focus_block", &json!({ "session_id": s, "block_id": b }).to_string()]);
}

#[test]
fn a_side_effect_never_puts_the_keys_on_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f, z, _tab2, x) = setup(&o);
    let p = |extra: Value| {
        let mut v = json!({ "session_id": s });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        v
    };
    let moves = [
        ("session.focus_next_pane", p(json!({ "wrap": true }))),
        ("session.focus_previous_pane", p(json!({ "wrap": true }))),
        ("session.focus_direction", p(json!({ "direction": "left", "wrap": true }))),
        ("session.focus_direction", p(json!({ "direction": "right", "wrap": true }))),
        ("session.toggle", p(json!({ "label": "worker" }))),
        // closing what has the keys: they go to X, never to W beside it
        ("block.close", p(json!({ "block_id": f }))),
        ("session.move_block", p(json!({ "block_id": x, "anchor_block_id": z }))),
    ];
    let got = from_inside(&o, &s, &moves);
    for ((m, params), (code, why, err)) in moves.iter().zip(&got) {
        assert!(!*why, "refused, where the rule moves the keys instead: {m} {params} (exit {code}): {err}");
    }
    let v = view(&o, &s);
    assert_eq!(keys_go_to(&v), x, "the keys went to the pane that isn't held: {v}");
    assert!(![w, z].contains(&keys_go_to(&v)));
}

#[test]
fn a_persons_ctrl_c_beside_the_prompt_does_not_hand_them_it() {
    let o = Orch::new();
    // W and F alone in their tab, X in another: F's pane closing would hand its keys to W
    let w = held(&o, &["--label", "worker"]);
    let s = o.env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let f = o.env.ok(&["split", "-b", &w, "--", "sh", "-c", "exec sleep 300"]);
    let x = o.env.ok(&["run", "-s", &s, "--", "sh", "-c", "exec sleep 300"]);
    focus(&o, &s, &f);
    // the person's own key, from outside leon: F's program ends and its pane closes
    o.env.ok(&["send-key", "-b", &f, "ctrl+c"]);
    let t = std::time::Instant::now();
    while view(&o, &s).to_string().contains(&f) {
        assert!(t.elapsed().as_secs() < 10, "F never closed");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let v = view(&o, &s);
    assert_ne!(keys_go_to(&v), w, "a pane closing handed the keys to the prompt: {v}");
    assert_eq!(keys_go_to(&v), x);
}

#[test]
fn a_program_cannot_steer_the_focus_onto_a_held_prompt() {
    let o = Orch::new();
    let (s, w, f, z, tab2, _x) = setup(&o);
    let p = |extra: Value| {
        let mut v = json!({ "session_id": s });
        v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        v
    };
    let refused = [
        ("session.focus_block", p(json!({ "block_id": w }))),
        ("session.focus_block", p(json!({ "block_id": z }))),
        ("session.focus_window", p(json!({ "window_id": tab2 }))),
        ("session.zoom_block", p(json!({ "block_id": w }))),
        ("session.float_block", p(json!({ "block_id": w }))),
        ("session.dock_block", p(json!({ "block_id": w, "side": "bottom" }))),
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
    let (s, w, f, _z, _tab2, _x) = setup(&o);
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
    let (s, w, _f, z, tab2, x) = setup(&o);
    // the test runs outside every leon terminal: a person, as the app and leon attach outside leon are
    focus(&o, &s, &w);
    assert_eq!(keys_go_to(&view(&o, &s)), w);
    // any move a person makes lands there: the previous pane from X beside it, or a direction
    let call = |m: &str, p: Value| o.env.ok(&["api", "call", m, &p.to_string()]);
    focus(&o, &s, &x);
    call("session.focus_previous_pane", json!({ "session_id": s, "wrap": true }));
    assert_eq!(keys_go_to(&view(&o, &s)), w);
    focus(&o, &s, &x);
    call("session.focus_direction", json!({ "session_id": s, "direction": "left", "wrap": true }));
    assert_eq!(keys_go_to(&view(&o, &s)), w, "{}", view(&o, &s));
    o.env.ok(&["api", "call", "session.focus_window", &json!({ "session_id": s, "window_id": tab2 }).to_string()]);
    assert_eq!(keys_go_to(&view(&o, &s)), z);
}
