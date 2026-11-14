//! The attached app's path (M10): the app's own client (`leon_app::remote`) against the release
//! server, next to the old in-process path (an embedded server, its grid polled every 8 ms as the
//! app did before M10). Budget: keystroke to screen p50 < 4 ms and p99 < 16 ms; `seq 1 200000`
//! at most 1.5x the in-process time.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use leon_app::embedded;
use leon_app::remote::Remote;
use serde_json::json;

const SEQ: &str = "read go; seq 1 200000; echo SEQ-END; exec sleep 300";
const SEQ_RUNS: usize = 3;
/// About the app's 1100×700 window in Menlo 13, as frame_bench sizes it.
const SIZE: (u16, u16) = (40, 110);
/// Time between keystrokes: faster than key repeat (about 30 a second), slower than the server's
/// 2 ms gap between two screens of one block, which only a flood should meet.
const KEY_GAP: Duration = Duration::from_millis(10);

pub struct Results {
    pub key: (f64, f64),
    pub seq_in_process: f64,
    pub seq_attached: f64,
}

impl Results {
    pub fn within_budget(&self) -> bool {
        self.key.0 < 4.0 && self.key.1 < 16.0 && self.seq_attached <= 1.5 * self.seq_in_process
    }
}

fn session(r: &Remote, command: &str) -> (String, String) {
    let options = json!({ "command": ["sh", "-c", command] });
    let made = r.call("session.create", json!({ "layout": { "block": { "options": options } } })).unwrap();
    let s = |k: &str| made[k].as_str().unwrap().to_string();
    (s("session_id"), s("block_id"))
}

/// Wait until block `id`'s screen satisfies `pred`, woken the way the app is (`on_change`).
fn until_screen(r: &Remote, changed: &Receiver<()>, id: &str, what: &str, pred: impl Fn(&str) -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while !r.model().screens.get(id).is_some_and(|f| pred(&f.text())) {
        assert!(Instant::now() < end, "timed out waiting for {what}");
        let _ = changed.recv_timeout(Duration::from_millis(100));
    }
}

/// Type one `x` at a time (`KEY_GAP` apart) into a raw-mode echo program and time until the
/// client has applied the pushed screen that shows it.
fn key_latency(r: &Remote, changed: &Receiver<()>, n: usize) -> (f64, f64) {
    assert!(n <= 1500, "the 24×80 screen holds about 1900 echoes");
    let (sid, bid) = session(r, "stty raw echo; echo READY; exec cat >/dev/null");
    r.watch(vec![bid.clone()]);
    until_screen(r, changed, &bid, "the raw-mode echo program", |t| t.contains("READY"));
    let times = (1..=n)
        .map(|i| {
            std::thread::sleep(KEY_GAP);
            let t = Instant::now();
            r.write(&sid, &bid, b"x");
            until_screen(r, changed, &bid, "an echo", |text| text.matches('x').count() == i);
            t.elapsed()
        })
        .collect();
    r.watch(vec![]);
    r.call("session.destroy", json!({ "session_id": sid })).unwrap();
    let (p50, p99) = super::percentiles(times);
    println!("leon attached keystroke to screen: samples={n} gap={KEY_GAP:?} p50={p50:.3} ms p99={p99:.3} ms");
    (p50, p99)
}

/// `seq 1 200000` from the go keystroke until the attached client's screen shows its end.
fn seq_attached(r: &Remote, changed: &Receiver<()>) -> f64 {
    let (sid, bid) = session(r, SEQ);
    r.resize(&sid, &bid, SIZE.0, SIZE.1);
    r.watch(vec![bid.clone()]);
    until_screen(r, changed, &bid, "the sized screen", |t| t.lines().count() == SIZE.0 as usize);
    let t = Instant::now();
    r.write(&sid, &bid, b"\r");
    until_screen(r, changed, &bid, "SEQ-END", |text| text.contains("SEQ-END"));
    let ms = t.elapsed().as_secs_f64() * 1e3;
    r.watch(vec![]);
    r.call("session.destroy", json!({ "session_id": sid })).unwrap();
    ms
}

/// The same in-process: the grid read under its lock every 8 ms, as the app did before M10.
fn seq_in_process(dir: &Path) -> f64 {
    let app = embedded::start_at(dir.join("in-process.sock")).unwrap();
    let options = json!({ "command": ["sh", "-c", SEQ] });
    let made = app.call("session.create", json!({ "layout": { "block": { "options": options } } })).unwrap();
    let id = made["block_id"].as_str().unwrap();
    let t = app.ctx().state().block(id).unwrap().1.terminal.clone();
    t.resize(SIZE.0, SIZE.1).unwrap();
    std::thread::sleep(Duration::from_millis(200)); // the shell reaches `read`
    let start = Instant::now();
    t.write(b"\r").unwrap();
    let (mut seen, end) = (u64::MAX, start + Duration::from_secs(30));
    loop {
        if t.generation() != seen {
            seen = t.generation();
            let f = t.with_term(|term| leon_app::render::frame(term, &leon_app::theme::DARK));
            if f.text().contains("SEQ-END") {
                break;
            }
        }
        assert!(Instant::now() < end, "in-process seq timed out");
        std::thread::sleep(Duration::from_millis(8));
    }
    let ms = start.elapsed().as_secs_f64() * 1e3;
    drop(t);
    app.stop();
    ms
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

pub fn run(socket: &Path, dir: &Path, samples: usize) -> Results {
    let r = Remote::connect(socket).expect("the bench attaches");
    let (tx, changed) = mpsc::channel();
    r.on_change(move || {
        let _ = tx.send(());
    });
    let key = key_latency(&r, &changed, samples.min(1500));
    // alternate the two paths, so the machine's load hits both alike
    let (mut inproc, mut attached) = (Vec::new(), Vec::new());
    for _ in 0..SEQ_RUNS {
        inproc.push(seq_in_process(dir));
        attached.push(seq_attached(&r, &changed));
    }
    let (seq_in_process, seq_attached) = (median(inproc.clone()), median(attached.clone()));
    println!("leon seq 1 200000 in-process (8 ms poll): median={seq_in_process:.0} ms all={inproc:.0?}");
    println!(
        "leon seq 1 200000 attached: median={seq_attached:.0} ms all={attached:.0?} ratio={:.2}",
        seq_attached / seq_in_process
    );
    Results { key, seq_in_process, seq_attached }
}

/// The attached rows of the table, and exit 1 when the budget is missed.
pub fn report(app: &Results) {
    println!("\n| attached app | keystroke to screen p50 / p99 | `seq 1 200000` in-process / attached |");
    println!("|---|---|---|");
    let ratio = app.seq_attached / app.seq_in_process;
    println!(
        "| leon | {:.3} / {:.3} ms | {:.0} / {:.0} ms ({ratio:.2}x) |",
        app.key.0, app.key.1, app.seq_in_process, app.seq_attached
    );
    let verdict = if app.within_budget() { "met" } else { "MISSED" };
    println!("\nbudget (p50 < 4 ms, p99 < 16 ms, seq at most 1.5x): {verdict}");
    if !app.within_budget() {
        std::process::exit(1);
    }
}
