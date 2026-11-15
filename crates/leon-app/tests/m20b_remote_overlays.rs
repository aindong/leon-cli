//! M20b criterion 1: the app's shared client keeps a window's floats and drawers, and block
//! labels, from the server's view; bounds a server should never send are dropped, not drawn.

use std::time::{Duration, Instant};

use leon_app::embedded;
use leon_app::remote::{self, Remote};
use leon_core::overlays::{DockSide, Overlays};
use serde_json::json;

fn private_dir() -> tempfile::TempDir {
    use std::os::unix::fs::PermissionsExt;
    tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap()
}

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn the_client_keeps_floats_drawers_and_labels() {
    let dir = private_dir();
    let app = embedded::start_at(dir.path().join("server.sock")).unwrap();
    let s = app.call("session.create", json!({})).unwrap();
    let sid = s["session_id"].as_str().unwrap().to_string();
    let bounds = json!({ "x": 0.2, "y": 0.1, "width": 0.5, "height": 0.6 });
    let f = app.call("session.new_floating", json!({ "session_id": sid, "bounds": bounds })).unwrap();
    let float = f["block_id"].as_str().unwrap().to_string();
    let f = app.call("session.new_floating", json!({ "session_id": sid })).unwrap();
    let top = f["block_id"].as_str().unwrap().to_string();
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch" })).unwrap();

    let remote = Remote::connect(app.socket()).unwrap();
    let got = || {
        let m = remote.model();
        let s = m.session(&sid)?.clone();
        let w = s.windows[s.active].id.clone();
        Some((s.overlay(&w), s))
    };
    until("the overlays", || got().is_some_and(|(o, _)| o.floating.len() == 2 && o.docked.len() == 1));
    let (o, s) = got().unwrap();
    let order: Vec<&str> = o.floating.iter().map(|f| f.block_id.as_str()).collect();
    assert_eq!(order, [float.as_str(), top.as_str()], "back to front");
    assert_eq!((o.floating[0].bounds.x, o.floating[0].bounds.width), (0.2, 0.5));
    assert!(!o.floating[0].hidden);
    let d = &o.docked[0];
    assert_eq!((d.side, d.size, d.hidden), (DockSide::Bottom, 0.3, true));
    let scratch = s.blocks.iter().find(|b| b.id == d.block_id).expect("the drawer's block");
    assert_eq!(scratch.label.as_deref(), Some("scratch"));

    // a change made elsewhere (the CLI) reaches the copy
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch" })).unwrap();
    until("the drawer shown", || got().is_some_and(|(o, _)| o.docked.first().is_some_and(|d| !d.hidden)));
    app.stop();
}

#[test]
fn bounds_a_server_should_never_send_give_no_overlays() {
    let w = json!({
        "window_id": "w",
        "floating": [{ "block_id": "a", "bounds": { "x": 0.5, "y": 0.0, "width": 2.0, "height": 0.5 }, "hidden": false }],
        "docked": [],
    });
    assert_eq!(remote::decode_overlays(&w), Overlays::default());
    let nan =
        json!({ "window_id": "w", "docked": [{ "block_id": "a", "side": "left", "size": 7.0, "hidden": false }] });
    assert_eq!(remote::decode_overlays(&nan), Overlays::default());
    // an older server sends neither field
    assert_eq!(remote::decode_overlays(&json!({ "window_id": "w" })), Overlays::default());
    // a newer server may add fields: the entry still decodes
    let newer = json!({
        "window_id": "w",
        "floating": [{ "block_id": "a", "bounds": { "x": 0.1, "y": 0.1, "width": 0.5, "height": 0.5, "z": 1 },
                       "hidden": false, "title": "x" }],
        "docked": [{ "block_id": "b", "side": "bottom", "size": 0.3, "hidden": true, "pinned": true }],
    });
    let o = remote::decode_overlays(&newer);
    assert_eq!((o.floating.len(), o.docked.len()), (1, 1), "{o:?}");
    assert!(o.docked[0].hidden);
    let ok =
        json!({ "window_id": "w", "docked": [{ "block_id": "a", "side": "left", "size": 0.25, "hidden": false }] });
    assert_eq!(remote::decode_overlays(&ok).docked.len(), 1);
}

#[test]
fn attach_reads_overlays_from_the_shared_client() {
    let attach = include_str!("../../leon/src/attach/view.rs");
    assert!(!attach.contains("\"session.view\""), "leon attach fetches session.view itself again");
}
