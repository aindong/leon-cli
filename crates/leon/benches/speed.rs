//! `cargo bench -p leon --bench speed [-- --samples N]`: input latency, output throughput and API
//! round trip against a private leon server, and the same against tmux when it is installed.
//! Results and method: docs/performance.md. Exits 1 when the attached app's budget is missed.

#[path = "speed/attached.rs"]
mod attached;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const THROUGHPUT_BYTES: u64 = 100_000_000;
const THROUGHPUT_RUNS: usize = 3;
const END: &str = "BENCH-END";
/// What the server's terminal must parse per run: 100 MB of `y\n`, each `\n` sent as `\r\n`, and the marker.
const PTY_BYTES: u64 = THROUGHPUT_BYTES / 2 * 3 + END.len() as u64;

/// One NDJSON connection to the server.
struct Conn {
    r: BufReader<UnixStream>,
    id: u64,
}

impl Conn {
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": self.id, "method": method, "params": params })).unwrap();
        line.push(b'\n');
        self.r.get_mut().write_all(&line).unwrap();
        let mut reply = String::new();
        self.r.read_line(&mut reply).unwrap();
        let v: Value = serde_json::from_str(&reply).unwrap_or_else(|e| panic!("{method}: bad reply {reply:?}: {e}"));
        assert!(v["error"].is_null(), "{method}: {}", v["error"]);
        v["result"].clone()
    }

    fn text(&mut self, sid: &Value, bid: &Value) -> String {
        let args = json!({ "session_id": sid, "block_id": bid, "method": "dev.leon.terminal.format", "args": { "trim": true } });
        self.call("block.method", args)["text"].as_str().unwrap().to_string()
    }

    /// A new session whose block runs `command`; returns (session id, block id).
    fn run(&mut self, command: &[&str], keep_open: bool) -> (Value, Value) {
        let options = json!({ "command": command, "exit": { "on_completion": !keep_open } });
        let r = self.call("session.create", json!({ "layout": { "block": { "options": options } } }));
        (r["session_id"].clone(), r["block_id"].clone())
    }
}

/// p50 and p99, in milliseconds.
fn percentiles(mut v: Vec<Duration>) -> (f64, f64) {
    v.sort();
    let at = |p: f64| v[((v.len() - 1) as f64 * p).round() as usize].as_secs_f64() * 1e3;
    (at(0.50), at(0.99))
}

/// Kills the server if the bench panics, so it can't outlive the run.
struct Server(std::process::Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn until(what: &str, mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(30);
    while !f() {
        assert!(Instant::now() < end, "timed out waiting for {what}");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let samples: usize =
        args.iter().position(|a| a == "--samples").and_then(|i| args.get(i + 1)?.parse().ok()).unwrap_or(300).max(1);
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let server = Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["server", "run", "--restore=false"])
        .env("LEON_RUNTIME_DIR", dir.path())
        .env("LEON_STATE_DIR", dir.path().join("state"))
        .env("LEON_CONFIG", dir.path().join("no-init.lua")) // never the developer's own init.lua
        .env("SHELL", "/bin/sh")
        .env_remove("LEON_SESSION")
        .env_remove("LEON_BLOCK")
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut server = Server(server);
    let socket = dir.path().join("server.sock");
    let mut stream = None;
    until("the server's socket", || {
        stream = UnixStream::connect(&socket).ok();
        stream.is_some()
    });
    let mut c = Conn { r: BufReader::new(stream.unwrap()), id: 0 };

    // The same 100 MB `yes | head` writes, written by `cat` in big blocks. macOS `head` writing to
    // a tty is line-buffered (one write per 2-byte line), which caps that run whatever reads it.
    let file = dir.path().join("yes.txt");
    std::fs::write(&file, "y\n".repeat(THROUGHPUT_BYTES as usize / 2)).unwrap();
    let cat = format!("cat '{}'", file.display());
    let yes = format!("yes | head -c {THROUGHPUT_BYTES}");

    let (rt, lat) = (round_trip(&mut c, samples * 5), latency(&mut c, samples));
    let leon = ("leon", rt, lat, throughput(&mut c, "yes | head", &yes), throughput(&mut c, "cat", &cat));
    let app = attached::run(&socket, dir.path(), samples);
    c.call("server.stop", json!({ "discard": true }));
    let _ = server.0.wait();

    let mut rows = vec![leon];
    match Command::new("tmux").arg("-V").output() {
        Ok(o) if o.status.success() => rows.push(tmux_row(samples, &yes, &cat)),
        _ => println!("tmux: skipped (not installed)"),
    }
    println!(
        "\n| | `session.list` round trip p50 / p99 | input latency p50 / p99 | `yes \\| head` 100 MB | `cat` 100 MB |"
    );
    println!("|---|---|---|---|---|");
    for (name, (rt50, rt99), (l50, l99), yes, cat) in rows {
        println!("| {name} | {rt50:.3} / {rt99:.3} ms | {l50:.3} / {l99:.3} ms | {yes:.1} MB/s | {cat:.1} MB/s |");
    }
    println!(
        "\n| attached app | keystroke to screen p50 / p99 | through a link p50 / p99 | `seq 1 200000` in-process / attached |"
    );
    println!("|---|---|---|---|");
    let ratio = app.seq_attached / app.seq_in_process;
    println!(
        "| leon | {:.3} / {:.3} ms | {:.3} / {:.3} ms | {:.0} / {:.0} ms ({ratio:.2}x) |",
        app.key.0, app.key.1, app.key_link.0, app.key_link.1, app.seq_in_process, app.seq_attached
    );
    let verdict = if app.within_budget() { "met" } else { "MISSED" };
    println!("\nbudget (p50 < 4 ms, p99 < 16 ms, seq at most 1.5x): {verdict}");
    if !app.within_budget() {
        std::process::exit(1);
    }
}

fn round_trip(c: &mut Conn, n: usize) -> (f64, f64) {
    (0..100).for_each(|_| drop(c.call("session.list", json!({}))));
    let times = (0..n)
        .map(|_| {
            let t = Instant::now();
            c.call("session.list", json!({}));
            t.elapsed()
        })
        .collect();
    let (p50, p99) = percentiles(times);
    println!("leon session.list round trip: samples={n} p50={p50:.3} ms p99={p99:.3} ms");
    (p50, p99)
}

/// Type one `x` at a time into a raw-mode PTY (the kernel echoes it) and time until the echo is
/// in the grid, polling the screen over the same connection.
fn latency(c: &mut Conn, n: usize) -> (f64, f64) {
    let (sid, bid) = c.run(&["sh", "-c", "stty raw echo; echo READY; exec cat >/dev/null"], false);
    until("the raw-mode echo program", || c.text(&sid, &bid).contains("READY"));
    let write =
        json!({ "session_id": sid, "block_id": bid, "method": "dev.leon.terminal.write", "args": { "data": "x" } });
    let times = (1..=n)
        .map(|i| {
            let t = Instant::now();
            c.call("block.method", write.clone());
            until("an echo", || c.text(&sid, &bid).matches('x').count() == i);
            t.elapsed()
        })
        .collect();
    c.call("session.destroy", json!({ "session_id": sid }));
    let (p50, p99) = percentiles(times);
    println!("leon input latency (send to echo in the grid): samples={n} p50={p50:.3} ms p99={p99:.3} ms");
    (p50, p99)
}

/// `writer` (100 MB) then an end marker; timed from creating the block until the server reports
/// the program exited, which it does only after parsing all its output. Then the marker must be on
/// the screen and the server's count of parsed bytes `PTY_BYTES` (a few more for `cat`, see below).
fn throughput(c: &mut Conn, what: &str, writer: &str) -> f64 {
    let script = format!("{writer}; printf '{END}'");
    let mut parsed = Vec::new();
    let mut rates: Vec<f64> = (0..THROUGHPUT_RUNS)
        .map(|_| {
            let t = Instant::now();
            let (sid, bid) = c.run(&["sh", "-c", &script], true);
            c.call("block.wait", json!({ "block_id": bid }));
            let secs = t.elapsed().as_secs_f64();
            assert!(c.text(&sid, &bid).ends_with(END), "the end marker reached the grid");
            let args = json!({ "session_id": sid, "block_id": bid, "method": "dev.leon.terminal.process" });
            let seen = c.call("block.method", args)["output_bytes"].as_u64().unwrap();
            // The macOS tty sends a \\r again when its queue fills between the \\r and \\n it makes of a
            // \\n, so a big-block writer arrives with a few extra \\r (about one per 1 KiB read).
            assert!(
                (PTY_BYTES..PTY_BYTES + PTY_BYTES / 100).contains(&seen),
                "the server parsed {seen} bytes, not {PTY_BYTES} (+ under 1% repeated \\r)"
            );
            parsed.push(seen);
            c.call("session.destroy", json!({ "session_id": sid }));
            THROUGHPUT_BYTES as f64 / 1e6 / secs
        })
        .collect();
    rates.sort_by(f64::total_cmp);
    let median = rates[rates.len() / 2];
    println!(
        "leon throughput ({what}): writer_bytes={THROUGHPUT_BYTES} server_parsed={parsed:?} \
         runs={THROUGHPUT_RUNS} median={median:.1} MB/s all={rates:.1?}"
    );
    median
}

type Row = (&'static str, (f64, f64), (f64, f64), f64, f64);

/// The same numbers through tmux's CLI (a process per call, as a script would drive it).
fn tmux_row(samples: usize, yes: &str, cat: &str) -> Row {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("tmux.sock");
    let tmux = |args: &[&str]| -> String {
        let out = Command::new("tmux").arg("-S").arg(&sock).args(["-f", "/dev/null"]).args(args).output().unwrap();
        assert!(out.status.success(), "tmux {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    };
    let screen = |t: &dyn Fn(&[&str]) -> String| t(&["capture-pane", "-p", "-S", "-", "-t", "b"]);
    tmux(&["new-session", "-d", "-s", "b", "-x", "80", "-y", "24", "stty raw echo; echo READY; exec cat >/dev/null"]);
    let times: Vec<Duration> = (0..samples * 5)
        .map(|_| {
            let t = Instant::now();
            tmux(&["list-sessions"]);
            t.elapsed()
        })
        .collect();
    let rt = percentiles(times);
    until("tmux's echo program", || screen(&tmux).contains("READY"));
    let times = (1..=samples)
        .map(|i| {
            let t = Instant::now();
            tmux(&["send-keys", "-t", "b", "-l", "x"]);
            until("a tmux echo", || screen(&tmux).matches('x').count() == i);
            t.elapsed()
        })
        .collect();
    let lat = percentiles(times);
    // Timed until the program finished writing; tmux may still be drawing the last of it.
    let rate = |writer: &str| {
        let script = format!("{writer}; printf '{END}'; tmux -S '{}' wait-for -S done", sock.display());
        let t = Instant::now();
        tmux(&["new-window", "-t", "b", &script]);
        tmux(&["wait-for", "done"]);
        THROUGHPUT_BYTES as f64 / 1e6 / t.elapsed().as_secs_f64()
    };
    let (yes, cat) = (rate(yes), rate(cat));
    let _ = tmux(&["kill-server"]);
    println!("tmux list-sessions round trip: samples={} p50={:.3} ms p99={:.3} ms", samples * 5, rt.0, rt.1);
    println!("tmux input latency: samples={samples} p50={:.3} ms p99={:.3} ms", lat.0, lat.1);
    println!("tmux throughput: writer_bytes={THROUGHPUT_BYTES} yes|head {yes:.1} MB/s, cat {cat:.1} MB/s");
    ("tmux (CLI)", rt, lat, yes, cat)
}
