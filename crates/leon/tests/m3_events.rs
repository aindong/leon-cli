//! M3 acceptance tests, criteria 8–10 (.agent/plans/2026-11-14-m3-layout-events.md): `leon events`
//! through the real binary. `Stream::next` fails on any seq gap, so every test also checks gaplessness.

mod support;

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use support::{revision, Env, Ev, Stream};

/// The next events, which must be exactly `names` in order.
fn expect(ev: &mut Stream, names: &[&str]) -> Vec<Ev> {
    let got: Vec<Ev> = names.iter().map(|_| ev.next()).collect();
    let got_names: Vec<&str> = got.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(got_names, names, "{got:?}");
    got
}

#[test]
fn c8_session_stream_has_every_layout_event_in_order() {
    let env = Env::new();
    let (s, a) = env.session();
    let mut ev = env.events(&["-s", &s]);

    let r = env.json(&["split", "-b", &a, "--json", "--", "sleep", "300"]);
    let b = r["block_id"].as_str().unwrap().to_string();
    let e = expect(&mut ev, &["block_created", "session_view_changed"]);
    assert_eq!(e[0].payload["block_id"], b.as_str());
    assert_eq!(e[0].session_id.as_deref(), Some(s.as_str()));
    assert_eq!(e[1].payload["revision"].as_u64(), Some(revision(&r)), "the event carries the call's revision");

    let w = env.ok(&["-s", &s, "window", "new"]);
    let e = expect(&mut ev, &["window_created", "active_window_changed", "block_created", "session_view_changed"]);
    assert_eq!(e[0].payload["window_id"], w.as_str());
    assert_eq!(e[1].payload["window_id"], w.as_str());

    env.ok(&["-s", &s, "window", "rename", "2", "logs"]);
    let e = expect(&mut ev, &["window_label_changed", "session_view_changed"]);
    assert_eq!((e[0].payload["window_id"].as_str(), e[0].payload["label"].as_str()), (Some(w.as_str()), Some("logs")));

    let r = env.json(&["-s", &s, "focus", "--json", &a]);
    let e = expect(&mut ev, &["active_window_changed", "session_view_changed"]);
    assert_ne!(e[0].payload["window_id"], w.as_str(), "focusing A activates its window");
    assert_eq!(e[1].payload["revision"].as_u64(), Some(revision(&r)));

    let r = env.json(&["zoom", "--json", &a]);
    let e = expect(&mut ev, &["session_view_changed"]);
    assert_eq!(e[0].payload["revision"].as_u64(), Some(revision(&r)));

    env.ok(&["block", "close", &b]);
    let e = expect(&mut ev, &["block_closed", "session_view_changed"]);
    assert_eq!(e[0].payload["block_id"], b.as_str());

    env.ok(&["-s", &s, "window", "close", "2"]);
    let e = expect(&mut ev, &["block_closed", "window_closed", "session_view_changed"]);
    assert_eq!(e[1].payload["window_id"], w.as_str());

    env.ok(&["session", "rename", &s, "renamed"]);
    let e = expect(&mut ev, &["session_label_changed", "session_view_changed"]);
    assert_eq!(e[0].payload["label"], "renamed");

    env.ok(&["kill", &s]);
    let e = expect(&mut ev, &["session_destroyed"]);
    assert_eq!(e[0].session_id.as_deref(), Some(s.as_str()));
    let end = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(st) = ev.child.try_wait().unwrap() {
            break st;
        }
        assert!(Instant::now() < end, "leon events -s kept running after its session ended");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(status.success(), "{status:?}");
}

#[test]
fn c8_text_mode() {
    let env = Env::new();
    let (s, a) = env.session();
    let mut child = env.cmd().args(["events", "-s", &s]).stdout(Stdio::piped()).spawn().unwrap();
    let (tx, lines) = mpsc::channel();
    let stdout = child.stdout.take().unwrap();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let first = lines.recv_timeout(Duration::from_secs(10)).expect("no first line");
    assert!(first.starts_with("attached at seq "), "{first}");
    env.ok(&["split", "-b", &a, "--", "sleep", "300"]);
    let line = lines.recv_timeout(Duration::from_secs(10)).expect("no event line");
    assert!(line.contains("block_created") && line.contains(&s), "{line}");
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn c9_lifecycle_and_all_scopes() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let mut life = env.events(&[]);
    let mut all = env.events(&["--all"]);

    let x = env.ok(&["new", "x"]);
    env.ok(&["-s", &x, "window", "new"]);
    env.ok(&["kill", &x]);

    let e = life.next();
    assert_eq!((e.name.as_str(), e.session_id.as_deref()), ("session_created", Some(x.as_str())), "{e:?}");
    assert_eq!(e.payload["session"]["label"], "x");
    let e = life.next();
    assert_eq!(
        (e.name.as_str(), e.session_id.as_deref()),
        ("session_destroyed", Some(x.as_str())),
        "the default scope has no window events: {e:?}"
    );

    let e = expect(
        &mut all,
        &[
            "session_created",
            "block_created",
            "session_view_changed",
            "window_created",
            "active_window_changed",
            "block_created",
            "session_view_changed",
            "session_destroyed",
        ],
    );
    assert!(e.iter().all(|e| e.session_id.as_deref() == Some(x.as_str())), "{e:?}");
}

#[test]
fn c10_terminal_events() {
    let env = Env::new();
    let (s, a) = env.session();
    let mut ev = env.events(&["-s", &s]);
    let script = r#"printf '\033]7;file://h/tmp\007\033]2;hello\007\007'; read x; exit 7"#;
    let r = env.json(&["split", "-b", &a, "--json", "--", "sh", "-c", script]);
    let b = r["block_id"].as_str().unwrap().to_string();

    let mut seen = HashMap::new();
    while seen.len() < 3 {
        let e = ev.next();
        if ["pwd_changed", "title_changed", "bell"].contains(&e.name.as_str()) {
            assert_eq!(e.block_id.as_deref(), Some(b.as_str()), "{e:?}");
            seen.insert(e.name.clone(), e.payload.clone());
        }
    }
    assert_eq!(seen["pwd_changed"]["pwd"], "file://h/tmp");
    assert_eq!(seen["title_changed"]["title"], "hello");

    env.ok(&["send-key", "-b", &b, "enter"]);
    let e = ev.until("child_exited");
    assert_eq!(e.block_id.as_deref(), Some(b.as_str()));
    assert_eq!(e.payload["exit_code"], 7);
    let e = ev.until("block_closed");
    assert_eq!(e.payload["block_id"], b.as_str());

    // Output right before an exit still comes first, and nothing about the block follows its close.
    let r = env.json(&["split", "-b", &a, "--json", "--", "sh", "-c", r"printf '\033]2;bye\007'; exit 3"]);
    let x = r["block_id"].as_str().unwrap().to_string();
    let about_x = |e: &Ev| e.block_id.as_deref() == Some(x.as_str()) || e.payload["block_id"] == x.as_str();
    let mut names = vec![];
    loop {
        let e = ev.next();
        if about_x(&e) {
            names.push(e.name.clone());
            if e.name == "title_changed" {
                assert_eq!(e.payload["title"], "bye");
            }
            if e.name == "child_exited" {
                assert_eq!(e.payload["exit_code"], 3);
            }
            if e.name == "block_closed" {
                break;
            }
        }
    }
    assert_eq!(names, ["block_created", "title_changed", "child_exited", "block_closed"]);
    let r = env.json(&["zoom", "--json", &a]);
    loop {
        let e = ev.next();
        assert!(!about_x(&e), "an event for a closed block: {e:?}");
        if e.name == "session_view_changed" && e.payload["revision"].as_u64() == Some(revision(&r)) {
            break;
        }
    }
}
