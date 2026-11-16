//! `LEON_FRAME_STATS=1`: how long frames take to build. `LEON_FRAME_LOG=PATH`: also every frame
//! and keystroke, one line each, for scripts/perf (docs/performance.md, "Measuring the app").

use std::io::Write;
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
    log: Option<Log>,
}

/// The per-frame log and what it is waiting for. Every time in it is the boot clock in
/// nanoseconds (CLOCK_UPTIME_RAW, which NSEvent's `timestamp` also counts in), so a script can
/// line it up with other processes' stamps.
struct Log {
    out: std::io::BufWriter<std::fs::File>,
    /// This frame's render start and its last pane's paint end.
    frame: Option<(u64, u64)>,
    /// The last key (when AppKit stamped it, when it was sent to the pane), the pane and that pane's cursor
    /// then: its echo is the first frame where that cursor moved.
    key: Option<(u64, u64, String, Cursor)>,
    /// The echo's frame: the key's times and that pane's paint end, logged at the next tick.
    echoed: Option<(u64, u64, u64)>,
    /// Whether a pane with text was drawn yet (start-up: the first prompt).
    text: bool,
}

/// A pane's cursor (line, column), if shown.
type Cursor = Option<(usize, usize)>;

/// The boot clock now, in nanoseconds.
pub(super) fn uptime_ns() -> u64 {
    const CLOCK_UPTIME_RAW: u32 = 8;
    unsafe extern "C" {
        fn clock_gettime_nsec_np(clock: u32) -> u64;
    }
    unsafe { clock_gettime_nsec_np(CLOCK_UPTIME_RAW) }
}

impl Stats {
    pub(super) fn new() -> Stats {
        let log = std::env::var_os("LEON_FRAME_LOG").and_then(|p| std::fs::File::create(p).ok());
        Stats::with(std::env::var("LEON_FRAME_STATS").is_ok_and(|v| v == "1"), log)
    }

    fn with(stats: bool, log: Option<std::fs::File>) -> Stats {
        let log =
            log.map(|f| Log { out: std::io::BufWriter::new(f), frame: None, key: None, echoed: None, text: false });
        Stats { enabled: stats || log.is_some(), log, ..Stats::default() }
    }

    /// Whether keys and paints are logged (`LEON_FRAME_LOG`), beyond the per-second summary.
    pub(super) fn logging(&self) -> bool {
        self.log.is_some()
    }

    /// Whether a paint should say if its pane has text: until the first one with text is logged.
    pub(super) fn wants_text(&self) -> bool {
        self.log.as_ref().is_some_and(|l| !l.text)
    }

    /// A key AppKit stamped at `stamped` (boot clock, ns) was sent to pane `block`,
    /// whose cursor is at `cursor`.
    pub(super) fn key(&mut self, stamped: u64, block: String, cursor: Option<(usize, usize)>) {
        let Some(log) = self.log.as_mut() else { return };
        let now = uptime_ns();
        // a stamp from the future is on another clock; a slow key (a stall) still counts
        if stamped > now {
            log.key = None;
            let _ = writeln!(log.out, "badclock {stamped} {now}");
            return;
        }
        // a key before the last one's echo: neither can be matched to its frame
        if log.key.take().is_some() {
            let _ = writeln!(log.out, "overlap {now}");
            return;
        }
        log.key = Some((stamped, now, block, cursor));
    }

    /// Pane `block` was painted with its cursor at `cursor`. True for the frame's first pane: the
    /// caller then asks for `presented` at the next display tick.
    pub(super) fn painted(&mut self, block: &str, cursor: Option<(usize, usize)>, has_text: bool) -> bool {
        let start = self.render_start.take();
        if let Some(start) = start {
            let took = start.elapsed();
            self.frames += 1;
            self.total += took;
            self.max = self.max.max(took);
            self.since.get_or_insert_with(Instant::now);
        }
        let Some(log) = self.log.as_mut() else { return false };
        let now = uptime_ns();
        match (start, log.frame.as_mut()) {
            (Some(start), _) => log.frame = Some((now - start.elapsed().as_nanos() as u64, now)),
            (None, Some((_, end))) => *end = now,
            (None, None) => {}
        }
        if log.key.as_ref().is_some_and(|(_, _, b, c)| b == block && *c != cursor) {
            let (stamped, handler, _, _) = log.key.take().unwrap();
            log.echoed = Some((stamped, handler, now));
        }
        if has_text && !log.text {
            log.text = true;
            let _ = writeln!(log.out, "text {now}");
        }
        start.is_some()
    }

    /// The display tick after the frame was submitted (not when it was presented: the drawable
    /// queue and the window server come after this).
    pub(super) fn presented(&mut self) {
        let Some(log) = self.log.as_mut() else { return };
        let now = uptime_ns();
        if let Some((start, end)) = log.frame.take() {
            let _ = writeln!(log.out, "frame {start} {end} {now}");
        }
        if let Some((stamped, handler, paint)) = log.echoed.take() {
            let _ = writeln!(log.out, "key {stamped} {handler} {paint} {now}");
        }
        let _ = log.out.flush();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(line: &str) -> Vec<u64> {
        line.split(' ').skip(1).map(|n| n.parse().unwrap()).collect()
    }

    #[test]
    fn off_without_the_env_and_a_key_is_timed_to_the_frame_that_moves_its_cursor() {
        let off = Stats::with(false, None);
        assert!(!off.enabled && off.log.is_none());

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("frames.log");
        let mut s = Stats::with(false, Some(std::fs::File::create(&path).unwrap()));
        assert!(s.enabled && s.logging() && s.wants_text());
        s.key(uptime_ns() - 2_000_000, "b1".into(), Some((0, 5)));
        // a frame that draws the pane unmoved, and another pane moving, are not the echo
        s.render_start = Some(Instant::now());
        assert!(s.painted("b1", Some((0, 5)), true));
        assert!(!s.painted("b2", Some((3, 1)), true));
        s.presented();
        s.render_start = Some(Instant::now());
        assert!(s.painted("b1", Some((0, 6)), true));
        s.presented();
        assert!(!s.wants_text(), "the first text is logged once");
        // a stamp from the future is refused; two keys before an echo are an overlap, unmatched
        s.key(uptime_ns() + 1_000_000_000, "b1".into(), Some((0, 6)));
        s.key(uptime_ns(), "b1".into(), Some((0, 6)));
        s.key(uptime_ns(), "b1".into(), Some((0, 6)));
        s.render_start = Some(Instant::now());
        s.painted("b1", Some((0, 7)), true);
        s.presented();

        let text = std::fs::read_to_string(&path).unwrap();
        let count = |p: &str| text.lines().filter(|l| l.starts_with(p)).count();
        assert_eq!(
            (count("text "), count("frame "), count("key "), count("overlap "), count("badclock ")),
            (1, 3, 1, 1, 1),
            "{text}"
        );
        for f in text.lines().filter(|l| l.starts_with("frame ")) {
            let [start, end, tick] = numbers(f)[..] else { panic!("{text}") };
            assert!(start <= end && end <= tick, "{text}");
        }
        let [stamped, handler, paint, tick] = numbers(text.lines().find(|l| l.starts_with("key ")).unwrap())[..] else {
            panic!("{text}")
        };
        assert!(handler - stamped >= 2_000_000 && handler <= paint && paint <= tick, "{text}");
    }
}
