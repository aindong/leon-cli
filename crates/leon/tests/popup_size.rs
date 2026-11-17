//! A popup's program starts at the popup's size (.agent/plans/2026-11-17-popup-size.md): once a
//! client has told the server a window's geometry (`client.geometry`), a float or drawer made there
//! spawns its PTY at the cells that client will give it, so a program like `less` never starts at
//! 24x80 and gets resized while it starts up.

mod support;

use serde_json::{json, Value};
use support::Env;

/// A pane area of 1000x800 px with 10x20 px cells. A float loses 16 px across and 40 px down to its
/// frame; a drawer 28 and 46 (its insets too).
fn geometry(sid: &str, wid: &str) -> Value {
    json!({ "session_id": sid, "window_id": wid, "area": [1000.0, 800.0], "cell": [10.0, 20.0],
            "float_frame": [16.0, 40.0], "drawer_frame": [28.0, 46.0] })
}

const STTY: [&str; 3] = ["sh", "-c", "stty size; exec sleep 300"];

/// A session with one tile; returns (session, window).
fn setup(e: &Env) -> (String, String) {
    let (sid, _) = e.session();
    let wid = e.window(&sid, 0)["window_id"].as_str().expect("a window id").to_string();
    (sid, wid)
}

fn report(e: &Env, g: &Value) -> i32 {
    e.code(&["api", "call", "client.geometry", &g.to_string()])
}

/// What block `label`'s program printed first: its `stty size`.
fn first_line(e: &Env, sid: &str, label: &str) -> String {
    let id = e.json(&["-s", sid, "block", "inspect", label, "--json"])["block_id"].as_str().unwrap().to_string();
    let screen = e.screen_until(&id, "stty size", |s| s.lines().any(|l| !l.trim().is_empty()));
    screen.lines().find(|l| !l.trim().is_empty()).unwrap().trim().to_string()
}

#[test]
fn a_popup_starts_at_its_float_size() {
    let e = Env::new();
    let (sid, wid) = setup(&e);
    assert_eq!(report(&e, &geometry(&sid, &wid)), 0);
    e.ok(&["-s", &sid, "toggle", "pop", "--create", "--float", "--", STTY[0], STTY[1], STTY[2]]);
    // the default bounds 0.1,0.1,0.8,0.8: 800x640 px, less the frame 784x600: 30 rows of 78
    assert_eq!(first_line(&e, &sid, "pop"), "30 78");
}

#[test]
fn a_drawer_starts_at_its_size() {
    let e = Env::new();
    let (sid, wid) = setup(&e);
    // a drawer on the left, before the bottom one, takes its width: 1000 - 250 = 750 px across
    let left = e.ok(&["-s", &sid, "split", "--", "sleep", "300"]);
    e.ok(&["-s", &sid, "-b", &left, "dock", "left", "--size", "0.25"]);
    assert_eq!(report(&e, &geometry(&sid, &wid)), 0);
    e.ok(&["-s", &sid, "toggle", "drawer", "--create", "--", STTY[0], STTY[1], STTY[2]]);
    // 0.3 of 800 px is 240 down, less 46: 9 rows; 750 less 28 across: 72 columns
    assert_eq!(first_line(&e, &sid, "drawer"), "9 72");
}

#[test]
fn new_floating_starts_at_its_size() {
    let e = Env::new();
    let (sid, wid) = setup(&e);
    assert_eq!(report(&e, &geometry(&sid, &wid)), 0);
    let layout = json!({ "block": { "label": "nf", "options": { "command": [STTY[0], STTY[1], STTY[2]] } } });
    let bounds = json!({ "x": 0.0, "y": 0.0, "width": 0.5, "height": 0.5 });
    let p = json!({ "session_id": sid, "window_id": wid, "bounds": bounds, "layout": layout });
    e.ok(&["api", "call", "session.new_floating", &p.to_string()]);
    // 500x400 px less the frame: 484x360, 18 rows of 48
    assert_eq!(first_line(&e, &sid, "nf"), "18 48");
}

#[test]
fn without_geometry_it_is_24x80() {
    let e = Env::new();
    let (sid, _) = setup(&e);
    e.ok(&["-s", &sid, "toggle", "pop", "--create", "--float", "--", STTY[0], STTY[1], STTY[2]]);
    assert_eq!(first_line(&e, &sid, "pop"), "24 80");
}

#[test]
fn bad_geometry_is_refused() {
    let e = Env::new();
    let (sid, wid) = setup(&e);
    let bad = |key: &str, v: Value| {
        let mut g = geometry(&sid, &wid);
        g[key] = v;
        g
    };
    for g in [
        bad("cell", json!([0.0, 20.0])),
        bad("cell", json!([10.0, -1.0])),
        bad("area", json!([-5.0, 800.0])),
        bad("float_frame", json!([16.0, -40.0])),
        bad("window_id", json!("window-nope")),
    ] {
        assert_ne!(report(&e, &g), 0, "refused: {g}");
    }
    // nothing was stored: still the default
    e.ok(&["-s", &sid, "toggle", "pop", "--create", "--float", "--", STTY[0], STTY[1], STTY[2]]);
    assert_eq!(first_line(&e, &sid, "pop"), "24 80");
}
