mod support;
use support::Env;

#[test]
fn m20_float_toggle_dock_and_live_hidden_shell() {
    let e = Env::new();
    let (sid, base) = e.session();
    let b = e.ok(&["-s", &sid, "float", "--new", "--label", "scratch", "--rect", "0.1,0.1,0.8,0.7", "--", "/bin/sh"]);
    let mut events = e.events(&["-s", &sid]);
    assert_eq!(e.window(&sid, 0)["floating"][0]["block_id"], b);
    let revision = e.view(&sid)["revision"].as_u64().unwrap();
    e.ok(&["-s", &sid, "toggle", "scratch"]);
    assert!(events.until("session_view_changed").payload["revision"].as_u64().unwrap() > revision);
    assert_eq!(e.window(&sid, 0)["floating"][0]["hidden"], true);
    e.ok(&["send", "-b", &b, "printf 'HIDDEN_%s\\n' OUTPUT\r"]);
    e.screen_until(&b, "hidden program output", |s| s.contains("HIDDEN_OUTPUT"));
    e.ok(&["-s", &sid, "toggle", "scratch"]);
    e.ok(&["dock", "-b", &b, "left", "--size", "0.25"]);
    assert_eq!(e.window(&sid, 0)["docked"][0]["side"], "left");
    e.ok(&["float", "-b", &b, "--unfloat"]);
    assert!(e.window(&sid, 0)["docked"].as_array().unwrap().is_empty());
    assert!(e.window(&sid, 0)["layout"].to_string().contains(&base));
    assert!(e.window(&sid, 0)["layout"].to_string().contains(&b));
    e.ok(&["-s", &sid, "toggle", "drawer", "--create"]);
    assert_eq!(e.window(&sid, 0)["docked"][0]["side"], "bottom");
}

#[test]
fn m20_attach_overlapping_floats_hide_and_input() {
    use support::host::{assert_restored, attach_then_stty, Host};
    let e = Env::new();
    let (sid, _) = e.session();
    e.ok(&["session", "rename", &sid, "m20attach"]);
    let lower = e.ok(&["-s", &sid, "float", "--new", "--label", "lower", "--", "/bin/sh"]);
    e.ok(&["send", "-b", &lower, "printf 'LOWER_%s\\n' MARK\r"]);
    e.screen_until(&lower, "lower marker", |s| s.contains("LOWER_MARK"));
    let upper = e.ok(&["-s", &sid, "float", "--new", "--label", "upper", "--", "/bin/sh"]);
    e.ok(&["send", "-b", &upper, "printf 'UPPER_%s\\n' MARK\r"]);
    e.screen_until(&upper, "upper marker", |s| s.contains("UPPER_MARK"));
    let h = Host::spawn(&e, &attach_then_stty("m20attach"), &[]);
    let frame = h.wait_text("UPPER_MARK");
    assert!(frame.contains('┌') && frame.contains('┘'), "float border: {frame}");
    assert!(!frame.contains("LOWER_MARK"), "upper obscures lower: {frame}");
    e.ok(&["-s", &sid, "toggle", "upper"]);
    let frame = h.wait_text("LOWER_MARK");
    assert!(!frame.contains("UPPER_MARK"));
    h.send(b"printf 'INPUT_%s\\n' LOWER\r");
    h.wait_text("INPUT_LOWER");
    e.screen_until(&lower, "input in visible float", |s| s.contains("INPUT_LOWER"));
    assert!(!e.capture(&upper).contains("INPUT_LOWER"));
    h.send(b"\x02d");
    assert_restored(&h, 0);
}

#[test]
fn m20_lua_helpers_and_scratch_key_recipe() {
    let e = Env::new();
    let (sid, _) = e.session();
    let script = e.dir.path().join("floats.lua");
    std::fs::write(
        &script,
        r#"
local f = assert(leon.float{new=true, layout={block={label="lua-float"}}})
assert(leon.toggle{label="lua-float"})
assert(leon.toggle{label="lua-float"})
assert(leon.dock{block_id=f.block_id, side="right", size=0.2})
assert(leon.float{block_id=f.block_id})
"#,
    )
    .unwrap();
    e.ok(&["-s", &sid, "do", script.to_str().unwrap()]);
    assert_eq!(e.window(&sid, 0)["floating"].as_array().unwrap().len(), 1);
    // This is the documented recipe, loaded by the server on reload.
    let docs = include_str!("../../../docs/lua/keys.md");
    let recipe = docs
        .split("<!-- m20-scratch -->")
        .nth(1)
        .unwrap()
        .split("```lua\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    std::fs::write(e.dir.path().join("no-init.lua"), recipe).unwrap();
    e.ok(&["config", "reload"]);
    let params = serde_json::json!({"session_id": sid, "key": "ctrl+g"}).to_string();
    e.ok(&["api", "call", "client.key", &params]);
    assert_eq!(e.window(&sid, 0)["docked"][0]["side"], "bottom");
    e.ok(&["api", "call", "client.key", &params]);
    assert_eq!(e.window(&sid, 0)["docked"][0]["hidden"], true);
}

#[test]
fn m20_invalid_requests_are_atomic_and_restore_preserves_hidden_placements() {
    let e = Env::new();
    let (sid, base) = e.session();
    for rect in ["0,0,2,1", "0,0,NaN,1", "0,0,1", "-0.1,0,1,1"] {
        let before = e.view(&sid);
        assert!(!e.run(&["-s", &sid, "float", "--new", "--rect", rect]).status.success());
        assert_eq!(e.view(&sid), before);
    }
    e.ok(&["float", "-b", &base, "--label", "saved"]);
    e.ok(&["-s", &sid, "toggle", "saved"]);
    e.ok(&["-s", &sid, "toggle", "drawer", "--create"]);
    let before = e.view(&sid);
    assert!(!e.run(&["dock", "-b", &base, "bottom"]).status.success());
    assert_eq!(e.view(&sid), before);
    assert!(!e.run(&["-s", &sid, "toggle", "other", "--create"]).status.success());
    assert_eq!(e.view(&sid), before);
    e.ok(&["-s", &sid, "toggle", "drawer"]);
    let before = e.view(&sid);
    e.ok(&["server", "stop"]);
    e.ok(&["server", "start"]);
    assert_eq!(e.view(&sid), before);
    assert!(e.window(&sid, 0)["layout"].is_null());
    assert_eq!(e.window(&sid, 0)["floating"][0]["hidden"], true);
}

#[test]
fn m20_hidden_output_still_streams_over_block_watch() {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::time::{Duration, Instant};
    let e = Env::new();
    let (sid, _) = e.session();
    let id = e.ok(&["-s", &sid, "float", "--new", "--label", "stream", "--", "/bin/sh"]);
    let mut sock = UnixStream::connect(e.socket()).unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    writeln!(sock, "{}", serde_json::json!({"id":1,"method":"block.watch","params":{"block_ids":[id]}})).unwrap();
    let mut reader = BufReader::new(sock);
    e.ok(&["-s", &sid, "toggle", "stream"]);
    e.ok(&["send", "-b", &id, "printf 'STREAM_%s\\n' HIDDEN\r"]);
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(Instant::now() < end, "hidden screen update deadline");
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0, "watch disconnected");
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v["screen"]["block_id"] == id && line.contains("STREAM_HIDDEN") {
            break;
        }
    }
}
