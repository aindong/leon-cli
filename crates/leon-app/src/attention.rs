//! What needs the user, across every machine (docs/spec.md §5 "UI use"): the Activity list's rows
//! by priority, the blocked count for the badges, and which state changes become a notification.
//! Pure: the window feeds it the models and the clock, and posts what it returns.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use leon_core::program_status::{strip_hidden, Kind};

use crate::remote::Model;
use crate::status::{self, Indicator};
use crate::switcher;

/// A run at least this long earns a notice when it finishes.
pub const LONG_RUN: Duration = Duration::from_secs(30);
/// One notice per block and state in this long, and at most `BURST` in all.
const QUIET: Duration = Duration::from_secs(60);
const BURST: usize = 4;
/// The longest title or body a notice shows, in chars.
const NOTICE_TEXT: usize = 120;

/// One block that needs the user, or is working.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Which machine (`Hosts` index; 0 is This Mac).
    pub host: usize,
    pub session_id: String,
    pub block_id: String,
    /// `host › session › tab`.
    pub place: String,
    pub status: Indicator,
    /// The program's title and msg, bidi-isolated (`status::caption_of`).
    pub caption: Option<String>,
    /// When it entered this state, as far as the app saw.
    pub since: Instant,
}

/// Blocked first, then error, done (unseen: focusing the pane clears it), working.
pub fn priority(i: Indicator) -> u8 {
    match i {
        Indicator::Blocked(_) => 0,
        Indicator::Error => 1,
        Indicator::Done => 2,
        Indicator::Working { .. } => 3,
    }
}

/// In priority order, and inside a state the one waiting longest first.
pub fn order(rows: &mut [Row]) {
    rows.sort_by_key(|r| (priority(r.status), r.since));
}

/// The state as a row says it: `blocked: question`, `working 40%`.
pub fn state_text(i: Indicator) -> String {
    match i {
        Indicator::Blocked(Some(k)) => format!("blocked: {}", kind_word(k)),
        Indicator::Blocked(None) => "blocked".into(),
        Indicator::Error => "error".into(),
        Indicator::Done => "done".into(),
        Indicator::Working { progress: Some(p) } => format!("working {p}%"),
        Indicator::Working { progress: None } => "working".into(),
    }
}

fn kind_word(k: Kind) -> &'static str {
    match k {
        Kind::Permission => "permission",
        Kind::Question => "question",
        Kind::Auth => "auth",
    }
}

/// How long a row has waited, short: `12s`, `4m`, `2h`.
pub fn waited(since: Instant, now: Instant) -> String {
    let s = now.saturating_duration_since(since).as_secs();
    match s {
        0..60 => format!("{s}s"),
        60..3600 => format!("{}m", s / 60),
        _ => format!("{}h", s / 3600),
    }
}

/// A badge's text: the count, nothing at 0.
pub fn badge(n: usize) -> Option<String> {
    match n {
        0 => None,
        1..=99 => Some(n.to_string()),
        _ => Some("99+".into()),
    }
}

/// A block entering a state (`Tracker::update`).
#[derive(Debug, Clone, PartialEq)]
pub struct Transition {
    pub host: usize,
    pub host_label: String,
    pub session_id: String,
    pub block_id: String,
    pub place: String,
    pub to: Indicator,
    pub caption: Option<String>,
    /// For `done`: how long it had been working, when the app saw it start.
    pub worked: Option<Duration>,
}

#[derive(Debug, Clone, Copy)]
struct Seen {
    rank: u8,
    since: Instant,
    working_since: Option<Instant>,
}

/// Each block's state as last seen, so the app knows how long it waited and what changed.
#[derive(Debug, Default)]
pub struct Tracker {
    seen: HashMap<(usize, String), Seen>,
    /// The machines seen once: a machine's first look reports no changes (what was already
    /// blocked when the app opened or a host connected is not news).
    known: Vec<usize>,
    rows: Vec<Row>,
}

impl Tracker {
    /// Look at every machine's model at `now`: rebuild the rows and return the blocks that entered
    /// a new state since the last look.
    pub fn update(&mut self, machines: &[(usize, &str, &Model)], now: Instant) -> Vec<Transition> {
        let mut changes = Vec::new();
        let mut rows = Vec::new();
        let mut seen = HashMap::new();
        for &(host, label, m) in machines {
            let first_look = !self.known.contains(&host);
            for (i, s) in m.sessions.iter().enumerate() {
                for b in &s.blocks {
                    let records = b.newest_first();
                    let Some(state) = status::indicator_of(&records) else { continue };
                    let tab = s.windows.iter().find(|w| w.id == b.window_id).map(|w| switcher::tab_title(s, w));
                    let place = format!("{label} › {} › {}", switcher::session_title(s, i), tab.unwrap_or_default());
                    let caption = status::caption_of(&records);
                    let key = (host, b.id.clone());
                    let rank = priority(state);
                    let was = self.seen.get(&key).copied();
                    let now_seen = match was {
                        Some(w) if w.rank == rank => w,
                        _ => {
                            let working = matches!(state, Indicator::Working { .. });
                            let working_since = if working { Some(now) } else { was.and_then(|w| w.working_since) };
                            if !first_look {
                                let worked = match state {
                                    Indicator::Done => was.and_then(|w| w.working_since).map(|t| now - t),
                                    _ => None,
                                };
                                changes.push(Transition {
                                    host,
                                    host_label: label.to_string(),
                                    session_id: s.id.clone(),
                                    block_id: b.id.clone(),
                                    place: place.clone(),
                                    to: state,
                                    caption: caption.clone(),
                                    worked,
                                });
                            }
                            Seen { rank, since: now, working_since }
                        }
                    };
                    seen.insert(key, now_seen);
                    let since = now_seen.since;
                    rows.push(Row {
                        host,
                        session_id: s.id.clone(),
                        block_id: b.id.clone(),
                        place,
                        status: state,
                        caption,
                        since,
                    });
                }
            }
            if first_look {
                self.known.push(host);
            }
        }
        // a machine that went away keeps nothing: when it comes back, its first look is quiet again
        self.known.retain(|h| machines.iter().any(|(m, _, _)| m == h));
        order(&mut rows);
        (self.seen, self.rows) = (seen, rows);
        changes
    }

    /// The Activity list: every block that needs the user, or is working, in order.
    pub fn rows(&self) -> Vec<Row> {
        self.rows.clone()
    }

    /// How many blocks are blocked (the badges).
    pub fn blocked(&self) -> usize {
        self.rows.iter().filter(|r| matches!(r.status, Indicator::Blocked(_))).count()
    }
}

/// Which notices are on (`leon.set("notifications.blocked", false)` and so on).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    pub blocked: bool,
    pub error: bool,
    pub done: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { blocked: true, error: true, done: true }
    }
}

/// A notification to post: text cleaned and short, and where a click goes (no block: Activity).
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub host_label: String,
    pub session_id: String,
    pub block_id: String,
}

/// Turns transitions into notices, rate-limited per block and state and in all; what a storm
/// holds back comes later as one notice.
#[derive(Debug, Default)]
pub struct Notifier {
    pub settings: Settings,
    last: HashMap<(usize, String, u8), Instant>,
    posted: VecDeque<Instant>,
    held: usize,
}

impl Notifier {
    /// The notice for `t`, if one is due. `active`: the app is in front; `on_screen`: the block is
    /// in the tab shown.
    pub fn offer(&mut self, t: &Transition, now: Instant, active: bool, on_screen: bool) -> Option<Notice> {
        let wanted = match t.to {
            Indicator::Blocked(_) => self.settings.blocked && !(active && on_screen),
            Indicator::Error => self.settings.error && !(active && on_screen),
            Indicator::Done => self.settings.done && !active && t.worked.is_some_and(|w| w >= LONG_RUN),
            Indicator::Working { .. } => false,
        };
        if !wanted {
            return None;
        }
        let key = (t.host, t.block_id.clone(), priority(t.to));
        if self.last.get(&key).is_some_and(|at| now.saturating_duration_since(*at) < QUIET) {
            return None;
        }
        self.last.insert(key, now);
        if !self.room(now) {
            self.held += 1;
            return None;
        }
        self.posted.push_back(now);
        let state = state_text(t.to);
        // the caption gives way, so the state always shows
        let body = match &t.caption {
            Some(c) => format!("{} ({state})", cut(&clean(c), NOTICE_TEXT - state.chars().count() - 3)),
            None => state,
        };
        Some(Notice {
            title: clean(&t.place),
            body,
            host_label: t.host_label.clone(),
            session_id: t.session_id.clone(),
            block_id: t.block_id.clone(),
        })
    }

    /// The one notice for what a storm held back, once there is room again.
    pub fn flush(&mut self, now: Instant) -> Option<Notice> {
        if self.held == 0 || !self.room(now) {
            return None;
        }
        self.posted.push_back(now);
        let n = std::mem::take(&mut self.held);
        Some(Notice {
            title: format!("{n} more blocks need you"),
            body: "See leon's Activity list".into(),
            host_label: String::new(),
            session_id: String::new(),
            block_id: String::new(),
        })
    }

    /// Whether another notice fits in the last minute's `BURST`.
    fn room(&mut self, now: Instant) -> bool {
        self.last.retain(|_, at| now.saturating_duration_since(*at) < QUIET); // only what can still hold one back
        while self.posted.front().is_some_and(|at| now.saturating_duration_since(*at) >= QUIET) {
            self.posted.pop_front();
        }
        self.posted.len() < BURST
    }
}

/// Program text made safe to show outside leon: on one line, nothing hidden (OSC 7501's own rule,
/// `strip_hidden`: controls, bidi controls, invisible and tag characters), at most `NOTICE_TEXT`
/// chars. Names set over the API or by a remote server never went through that rule.
pub fn clean(s: &str) -> String {
    let one_line = s.replace(['\u{2028}', '\u{2029}'], " ");
    cut(&strip_hidden(&one_line), NOTICE_TEXT)
}

/// `s` in at most `most` chars, an ellipsis ending one that was longer.
fn cut(s: &str, most: usize) -> String {
    if s.chars().count() <= most {
        return s.to_string();
    }
    let mut cut: String = s.chars().take(most.saturating_sub(1)).collect();
    cut.push('…');
    cut
}
