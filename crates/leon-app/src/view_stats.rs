//! `LEON_FRAME_STATS=1`: how long frames take to build.

use std::time::{Duration, Instant};

/// Frame build times (render start to paint end), printed each second that drew when
/// `LEON_FRAME_STATS=1`.
#[derive(Default)]
pub(super) struct Stats {
    pub(super) enabled: bool,
    pub(super) render_start: Option<Instant>,
    frames: u32,
    total: Duration,
    max: Duration,
    since: Option<Instant>,
}

impl Stats {
    pub(super) fn new() -> Stats {
        Stats { enabled: std::env::var("LEON_FRAME_STATS").is_ok_and(|v| v == "1"), ..Stats::default() }
    }

    pub(super) fn painted(&mut self) {
        if let Some(start) = self.render_start.take() {
            let took = start.elapsed();
            self.frames += 1;
            self.total += took;
            self.max = self.max.max(took);
            self.since.get_or_insert_with(Instant::now);
        }
    }

    pub(super) fn report(&mut self) {
        let Some(since) = self.since.filter(|s| s.elapsed() >= Duration::from_secs(1)) else { return };
        let ms = |d: Duration| d.as_secs_f64() * 1000.;
        eprintln!(
            "leon frames: {} in {:.2}s, avg {:.2} ms, max {:.2} ms",
            self.frames,
            since.elapsed().as_secs_f64(),
            ms(self.total) / self.frames as f64,
            ms(self.max)
        );
        (self.frames, self.total, self.max, self.since) = (0, Duration::ZERO, Duration::ZERO, None);
    }
}
