use std::os::unix::fs::PermissionsExt;

use serde_json::json;

use super::*;
use crate::Info;

fn ctx() -> Arc<Ctx> {
    Arc::new(Ctx::new(Info { pid: 1, version: "test", socket: "/tmp/x".into() }, false))
}

/// A saved file's JSON for one session with a split window, from a live server state.
async fn saved_json() -> Value {
    let c = ctx();
    let sleep = json!({ "block": { "label": "a", "options": { "command": ["sleep", "300"] } } });
    let r = c.call("session.create", json!({ "label": "s", "layout": sleep })).await.unwrap();
    let (sid, a) = (r["session_id"].clone(), r["block_id"].clone());
    let split = json!({
        "session_id": sid, "anchor_id": a, "direction": "vertical", "side": "after", "ratio": 0.3,
        "layout": { "block": { "options": { "command": ["sleep", "300"] } } },
    });
    c.call("session.new_split", split).await.unwrap();
    let saved = snapshot(&c.state());
    serde_json::to_value(saved).unwrap()
}

fn check_json(v: &Value) -> Result<(), String> {
    check(&serde_json::from_value::<Saved>(v.clone()).map_err(|e| e.to_string())?)
}

#[tokio::test]
async fn a_snapshot_passes_its_own_checks() {
    let v = saved_json().await;
    assert_eq!(v["version"], 1);
    assert_eq!(v["sessions"][0]["blocks"][1]["command"], json!(["sleep", "300"]), "saved for the record");
    check_json(&v).unwrap();
}

#[tokio::test]
async fn checks_refuse_what_the_model_would_trip_on() {
    let good = saved_json().await;
    type Break = fn(&mut Value);
    let broken: Vec<(&str, Break)> = vec![
        ("duplicate ID", |v| {
            v["sessions"][0]["blocks"][1]["block_id"] = v["sessions"][0]["blocks"][0]["block_id"].clone()
        }),
        ("bad ratio", |v| v["sessions"][0]["windows"][0]["layout"]["split"]["ratio"] = json!(1.5)),
        ("leaf without a block", |v| v["sessions"][0]["blocks"].as_array_mut().unwrap().truncate(1)),
        ("focus outside", |v| v["sessions"][0]["windows"][0]["focus"] = json!("block:elsewhere")),
        ("active past the end", |v| v["sessions"][0]["active"] = json!(1)),
        ("ID with an escape", |v| v["sessions"][0]["session_id"] = json!("session:\u{1b}[2J")),
        ("label with a newline", |v| v["sessions"][0]["label"] = json!("a\nb")),
        ("block in no window", |v| v["sessions"][0]["blocks"][1]["window_id"] = json!("window:gone")),
    ];
    for (what, f) in broken {
        let mut v = good.clone();
        f(&mut v);
        assert!(check_json(&v).is_err(), "{what} passed the checks");
    }
}

#[tokio::test]
async fn save_is_private_and_atomic_and_load_reads_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let c = ctx();
    c.call("session.create", json!({ "label": "kept" })).await.unwrap();
    save(&c.state(), &file).unwrap();
    assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
    let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, [FILE], "no temp file left");
    let saved = load(&file).unwrap().unwrap();
    assert_eq!(saved.sessions[0].label.as_deref(), Some("kept"));
}

#[test]
fn newer_and_missing_versions_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    assert!(load(&file).unwrap().is_none(), "no file: nothing to restore");
    fs::write(&file, br#"{"version":2,"sessions":[]}"#).unwrap();
    assert!(load(&file).err().unwrap().contains("newer"));
    fs::write(&file, br#"{"sessions":[]}"#).unwrap();
    assert!(load(&file).is_err());
}

#[test]
fn moving_aside_never_replaces_an_earlier_bad_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    for bytes in [b"first".as_slice(), b"second"] {
        fs::write(&file, bytes).unwrap();
        aside(&file, "test");
    }
    let mut kept: Vec<Vec<u8>> =
        fs::read_dir(dir.path()).unwrap().map(|e| fs::read(e.unwrap().path()).unwrap()).collect();
    kept.sort();
    assert_eq!(kept, [b"first".to_vec(), b"second".to_vec()], "both kept, the file itself gone");
}

#[test]
fn ratios_come_back_bit_for_bit() {
    // A ratio one ULP off could become 1.0 and fail check_ratio, which would set the whole file aside.
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut ratios = vec![0.3, 0.7, 1e-300, f64::MIN_POSITIVE, 0.999_999_999_999_999_9, 1.0 - f64::EPSILON / 2.0];
    for _ in 0..100_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        ratios.push((x >> 11) as f64 / (1u64 << 53) as f64);
    }
    for r in ratios.into_iter().filter(|r| *r > 0.0 && *r < 1.0) {
        let back: f64 = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert_eq!(back.to_bits(), r.to_bits(), "{r:e} came back as {back:e}");
    }
}
