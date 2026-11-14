//! Impact proof (.agent/plans/2026-11-14-m3-layout-events.md): an attach snapshot sits exactly
//! between two events, even while another connection changes the session as fast as it can. The
//! first view change after a snapshot at revision R is R + 1, with seq = snapshot seq + 2 (a label
//! change, then its view change): nothing is missed and nothing the snapshot holds is repeated.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

fn call(r: &mut BufReader<UnixStream>, method: &str, params: Value) -> (Value, Vec<Value>) {
    writeln!(r.get_mut(), "{}", json!({ "id": 1, "method": method, "params": params })).unwrap();
    let mut early = vec![];
    loop {
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        if v.get("event").is_some() {
            early.push(v);
        } else {
            return (v["result"].clone(), early);
        }
    }
}

#[test]
fn snapshots_and_events_line_up_under_concurrent_changes() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let info = leon_server::Info { pid: 1, version: "test", socket: socket.display().to_string() };
    let ctx = Arc::new(leon_server::Ctx::new(info, false));
    let opts =
        leon_server::serve::Options { socket: socket.clone(), idle_exit: Duration::from_secs(60), persist: None };
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.spawn(leon_server::serve::serve(ctx, opts));
    let connect = || loop {
        if let Ok(s) = UnixStream::connect(&socket) {
            s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
            return BufReader::new(s);
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut c = connect();
    let layout = json!({ "block": { "options": { "command": ["sleep", "60"] } } });
    let sid = call(&mut c, "session.create", json!({ "layout": layout })).0["session_id"].as_str().unwrap().to_string();

    let stop = Arc::new(AtomicBool::new(false));
    let (busy, s2, mut c2) = (stop.clone(), sid.clone(), connect());
    let writer = std::thread::spawn(move || {
        let mut i = 0;
        while !busy.load(Ordering::SeqCst) {
            i += 1;
            call(&mut c2, "session.set_label", json!({ "session_id": s2, "label": format!("l{i}") }));
        }
    });
    for _ in 0..100 {
        let mut a = connect();
        let (snap, early) = call(&mut a, "session.attach", json!({ "session_id": sid }));
        let (seq, rev) = (snap["seq"].as_u64().unwrap(), snap["view"]["revision"].as_u64().unwrap());
        let mut events = early.into_iter();
        let mut next = || {
            events.next().unwrap_or_else(|| {
                let mut line = String::new();
                a.read_line(&mut line).unwrap();
                serde_json::from_str(&line).unwrap()
            })
        };
        let label = next();
        let view = next();
        assert_eq!(label["seq"].as_u64(), Some(seq + 1), "{label}");
        assert_eq!(view["seq"].as_u64(), Some(seq + 2), "{view}");
        let inner = &view["event"]["payload"]["event"];
        assert_eq!(inner["type"], "session_view_changed", "{view}");
        assert_eq!(inner["payload"]["revision"].as_u64(), Some(rev + 1), "snapshot at {rev}, then {view}");
    }
    stop.store(true, Ordering::SeqCst);
    writer.join().unwrap();
}
