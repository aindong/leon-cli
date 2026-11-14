//! Headless frame-cost bench: runs `seq 1 200000` (or the given command) in a terminal sized like
//! the app's default window and, at the app's 8 ms poll rate, builds the render model whenever
//! the screen changed. Reports wall time and the model build time per frame. GPU work and text
//! shaping are not included; `LEON_FRAME_STATS=1` on the real app measures those.
//!
//! cargo run --release -p leon-app --example frame_bench [-- CMD ARGS...]

use std::time::{Duration, Instant};

use leon_app::{render, theme};
use leon_core::terminal::{Spec, Terminal};

fn main() {
    let mut command: Vec<String> = std::env::args().skip(1).collect();
    if command.is_empty() {
        command = ["seq", "1", "200000"].map(String::from).to_vec();
    }
    let (tx, rx) = std::sync::mpsc::channel();
    let spec = Spec { command: command.clone(), cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, move |code| {
        let _ = tx.send(code);
    })
    .expect("spawn");
    t.resize(40, 110).expect("resize"); // about the app's 1100×700 window in Menlo 13
    let start = Instant::now();
    let (mut seen, mut times) = (u64::MAX, Vec::new());
    loop {
        let done = rx.try_recv().is_ok();
        let generation = t.generation();
        if generation != seen {
            seen = generation;
            let s = Instant::now();
            let f = t.with_term(|term| render::frame(term, &theme::DARK));
            times.push(s.elapsed());
            std::hint::black_box(f);
        }
        if done {
            break;
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    let wall = start.elapsed();
    times.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.;
    let total: Duration = times.iter().sum();
    println!(
        "{}: {:.0} ms wall, {} frames, model build avg {:.3} ms, p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms",
        command.join(" "),
        ms(wall),
        times.len(),
        ms(total) / times.len().max(1) as f64,
        ms(times[times.len() / 2]),
        ms(times[times.len() * 99 / 100]),
        ms(*times.last().unwrap()),
    );
    println!("last line: {:?}", t.text(true).lines().last().unwrap_or(""));
}
