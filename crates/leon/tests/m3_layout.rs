//! M3 acceptance tests, criteria 1–7 (.agent/plans/2026-11-14-m3-layout-events.md): the layout
//! tree through the real `leon` binary, read back with `session inspect --json`.

mod support;

use serde_json::Value;
use support::{revision, shape, Env};

fn split(env: &Env, args: &[&str]) -> (String, u64) {
    let mut all = vec!["split", "--json"];
    all.extend_from_slice(args);
    all.extend_from_slice(&["--", "sleep", "300"]);
    let r = env.json(&all);
    (r["block_id"].as_str().unwrap_or_else(|| panic!("split printed no block_id: {r}")).to_string(), revision(&r))
}

fn layout(env: &Env, s: &str) -> Value {
    env.window(s, 0)["layout"].clone()
}

fn focused(env: &Env, s: &str) -> String {
    env.window(s, 0)["focused_block_id"].as_str().unwrap_or_default().to_string()
}

fn view_revision(env: &Env, s: &str) -> u64 {
    revision(&env.view(s))
}

/// A | (B / C): A on the left; B over C on the right, with C focused.
fn three(env: &Env) -> (String, String, String, String) {
    let (s, a) = env.session();
    let (b, _) = split(env, &["-b", &a]);
    let (c, _) = split(env, &["-b", &b, "--split", "below"]);
    (s, a, b, c)
}

#[test]
fn c1_split_builds_the_tree() {
    let env = Env::new();
    let (s, a) = env.session();
    let before = view_revision(&env, &s);
    let (b, r1) = split(&env, &["-b", &a]);
    assert!(r1 > before, "split returns a higher revision ({before} -> {r1})");
    let l = layout(&env, &s);
    let sp = &l["split"];
    assert_eq!(sp["direction"], "horizontal", "{l}");
    assert!((sp["ratio"].as_f64().unwrap() - 0.5).abs() < 1e-9, "{l}");
    assert!(sp["split_id"].as_str().unwrap().starts_with("split:"), "{l}");
    assert_eq!(shape(&l, &[(&a, "A"), (&b, "B")]), "h(A,B)");
    assert_eq!(focused(&env, &s), b, "the CLI focuses the new pane");

    let (c, r2) = split(&env, &["-b", &b, "--split", "below", "--ratio", "30"]);
    assert!(r2 > r1);
    let l = layout(&env, &s);
    assert_eq!(shape(&l, &[(&a, "A"), (&b, "B"), (&c, "C")]), "h(A,v(B,C))");
    let inner = &l["split"]["after"]["split"];
    assert_eq!(inner["direction"], "vertical");
    assert!((inner["ratio"].as_f64().unwrap() - 0.7).abs() < 1e-9, "C gets 30 %: {l}");
    assert_ne!(inner["split_id"], l["split"]["split_id"], "each split has its own id");

    let (d, _) = split(&env, &["-b", &a, "--split", "left", "--ratio", "30"]);
    let l = layout(&env, &s);
    assert!((l["split"]["before"]["split"]["ratio"].as_f64().unwrap() - 0.3).abs() < 1e-9, "D (before) gets 30 %: {l}");
    let (e, _) = split(&env, &["-b", &c, "--split", "above"]);
    let names = [(a.as_str(), "A"), (b.as_str(), "B"), (c.as_str(), "C"), (d.as_str(), "D"), (e.as_str(), "E")];
    assert_eq!(shape(&layout(&env, &s), &names), "h(h(D,A),v(B,v(E,C)))");

    // A split is an anchor too: splitting beside the root wraps the whole tree.
    let root = layout(&env, &s)["split"]["split_id"].as_str().unwrap().to_string();
    let (f, _) = split(&env, &["-b", &root, "--split", "below"]);
    let mut names = names.to_vec();
    names.push((f.as_str(), "F"));
    assert_eq!(shape(&layout(&env, &s), &names), "v(h(h(D,A),v(B,v(E,C))),F)");
}

#[test]
fn c2_focus_by_target_direction_and_order() {
    let env = Env::new();
    let (s, a, b, c) = three(&env);
    assert_eq!(focused(&env, &s), c);
    let steps: [(&[&str], &str); 9] = [
        (&["up"], &b),
        (&["left"], &a),
        (&["left"], &a),
        (&["left", "--wrap"], &b),
        (&["next"], &c),
        (&["next"], &c),
        (&["next", "--wrap"], &a),
        (&["prev", "--wrap"], &c),
        (&[&c], &c),
    ];
    let mut was = focused(&env, &s);
    let mut rev = view_revision(&env, &s);
    for (args, want) in steps {
        let mut all = vec!["-s", s.as_str(), "focus", "--json"];
        all.extend_from_slice(args);
        let r = env.json(&all);
        let now = focused(&env, &s);
        assert_eq!(now, *want, "focus {args:?}");
        if now == was {
            assert_eq!(revision(&r), rev, "focus {args:?} changed nothing, so no new revision");
        } else {
            assert!(revision(&r) > rev, "focus {args:?} changed focus, so a new revision");
        }
        rev = revision(&r);
        was = now;
    }
}

#[test]
fn c3_move_collapses_and_reinserts() {
    let env = Env::new();
    let (s, a, b, c) = three(&env);
    env.ok(&["move", &a, "below", &c, "--ratio", "30"]);
    let names = [(a.as_str(), "A"), (b.as_str(), "B"), (c.as_str(), "C")];
    let l = layout(&env, &s);
    assert_eq!(shape(&l, &names), "v(B,v(C,A))");
    assert!((l["split"]["after"]["split"]["ratio"].as_f64().unwrap() - 0.7).abs() < 1e-9, "A (after) gets 30 %: {l}");
    assert_eq!(env.code(&["move", &a, "right", &a]), 1, "a block can't move beside itself");

    let d = env.ok(&["-s", &s, "run", "--", "sleep", "300"]);
    assert_eq!(env.view(&s)["windows"].as_array().unwrap().len(), 2);
    env.ok(&["move", &d, "right", &a]);
    let v = env.view(&s);
    assert_eq!(v["windows"].as_array().unwrap().len(), 1, "the emptied window closed: {v}");
    let names = [(a.as_str(), "A"), (b.as_str(), "B"), (c.as_str(), "C"), (d.as_str(), "D")];
    assert_eq!(shape(&v["windows"][0]["layout"], &names), "v(B,v(C,h(A,D)))");
}

#[test]
fn c4_swap_trades_places() {
    let env = Env::new();
    let (s, a, b, c) = three(&env);
    env.ok(&["focus", "-s", &s, &a]);
    env.ok(&["swap", &b, &a]);
    let names = [(a.as_str(), "A"), (b.as_str(), "B"), (c.as_str(), "C")];
    assert_eq!(shape(&layout(&env, &s), &names), "h(B,v(A,C))");
    assert_eq!(focused(&env, &s), b, "focus stays at the left position");

    let d = env.ok(&["-s", &s, "run", "--", "sleep", "300"]);
    env.ok(&["swap", &d, &c]);
    let v = env.view(&s);
    let names = [(a.as_str(), "A"), (b.as_str(), "B"), (c.as_str(), "C"), (d.as_str(), "D")];
    assert_eq!(shape(&v["windows"][0]["layout"], &names), "h(B,v(A,D))");
    assert_eq!(shape(&v["windows"][1]["layout"], &names), "C");
    let blocks = env.json(&["-s", &s, "block", "ls", "--json"]);
    let window_of =
        |id: &str| blocks.as_array().unwrap().iter().find(|x| x["block_id"] == id).unwrap()["window_id"].clone();
    assert_eq!(window_of(&c), v["windows"][1]["window_id"], "C moved to the second window");
    assert_eq!(window_of(&d), v["windows"][0]["window_id"]);
}

#[test]
fn c5_zoom_is_display_only() {
    let env = Env::new();
    let (s, _a, b, _c) = three(&env);
    let tree = layout(&env, &s);
    env.ok(&["zoom", &b]);
    let w = env.window(&s, 0);
    assert_eq!(w["zoomed_block_id"], b.as_str());
    assert_eq!(w["layout"], tree, "zoom leaves the layout alone");
    env.ok(&["zoom", &b]);
    assert!(env.window(&s, 0)["zoomed_block_id"].is_null(), "zoom toggles");
    env.ok(&["zoom", "--on", &b]);
    env.ok(&["zoom", "--on", &b]);
    assert_eq!(env.window(&s, 0)["zoomed_block_id"], b.as_str(), "--on twice stays on");
    env.ok(&["zoom", "--off", &b]);
    assert!(env.window(&s, 0)["zoomed_block_id"].is_null());
}

#[test]
fn c6_resize_sets_the_parent_split() {
    let env = Env::new();
    let (s, a, b, c) = three(&env);
    env.ok(&["resize", &b, "--ratio", "25"]);
    let l = layout(&env, &s);
    assert!((l["split"]["after"]["split"]["ratio"].as_f64().unwrap() - 0.25).abs() < 1e-9, "{l}");
    env.ok(&["resize", &c, "--ratio", "25"]);
    let l = layout(&env, &s);
    assert!((l["split"]["after"]["split"]["ratio"].as_f64().unwrap() - 0.75).abs() < 1e-9, "C is after: {l}");
    env.ok(&["resize", &a, "--ratio", "60"]);
    let l = layout(&env, &s);
    assert!((l["split"]["ratio"].as_f64().unwrap() - 0.6).abs() < 1e-9, "{l}");
    assert_eq!(env.code(&["resize", &a, "--ratio", "0"]), 2);
    assert_eq!(env.code(&["resize", &a, "--ratio", "100"]), 2);
}

#[test]
fn c7_close_collapses_the_tree() {
    let env = Env::new();
    let (s, a, b, c) = three(&env);
    assert_eq!(focused(&env, &s), c);
    env.ok(&["block", "close", &c]);
    let names = [(a.as_str(), "A"), (b.as_str(), "B"), (c.as_str(), "C")];
    assert_eq!(shape(&layout(&env, &s), &names), "h(A,B)");
    assert_eq!(focused(&env, &s), b, "focus moves to the sibling");
    env.ok(&["block", "close", &b]);
    assert_eq!(shape(&layout(&env, &s), &names), "A");
    assert_eq!(focused(&env, &s), a);
    assert_eq!(env.code(&["split", "-b", "nope", "--", "sleep", "1"]), 3);
}
