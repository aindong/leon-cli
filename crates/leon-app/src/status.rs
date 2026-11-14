//! What the app shows for OSC 7501 program status (docs/spec.md §5 "UI use"): one indicator per
//! pane, tab or session, the highest-priority state among its records, and a pane caption.

use leon_core::program_status::{Kind, Record, State};

use crate::theme::Theme;

/// A pane's, tab's or session's status, as drawn. `idle` draws nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Indicator {
    Working { progress: Option<u8> },
    Blocked(Option<Kind>),
    Done,
    Error,
}

/// The longest caption shown, in chars; records allow 192 (title) and 2048 (msg).
const CAPTION: usize = 120;
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl Indicator {
    /// What needs the user most comes first: blocked > error > working > done.
    fn rank(self) -> u8 {
        match self {
            Indicator::Done => 1,
            Indicator::Working { .. } => 2,
            Indicator::Error => 3,
            Indicator::Blocked(_) => 4,
        }
    }

    fn of(r: &Record) -> Option<Indicator> {
        Some(match r.state {
            State::Idle => return None,
            State::Working => Indicator::Working { progress: r.progress },
            State::Blocked => Indicator::Blocked(r.kind),
            State::Done => Indicator::Done,
            State::Error => Indicator::Error,
        })
    }

    pub fn color(self, t: &Theme) -> u32 {
        match self {
            Indicator::Working { .. } => t.accent,
            Indicator::Blocked(_) => t.ansi[3],
            Indicator::Done => t.ansi[2],
            Indicator::Error => t.ansi[1],
        }
    }
}

/// The highest-priority indicator; on a tie the first.
pub fn aggregate(all: impl IntoIterator<Item = Option<Indicator>>) -> Option<Indicator> {
    all.into_iter().flatten().fold(None, |best, i| match best {
        Some(b) if b.rank() >= i.rank() => Some(b),
        _ => Some(i),
    })
}

/// One terminal's indicator from its records (least recently reported first).
pub fn indicator(records: &[(Option<String>, Record)]) -> Option<Indicator> {
    indicator_of(&newest_first(records))
}

/// One terminal's indicator from its records, most recently reported first.
pub fn indicator_of(newest_first: &[&Record]) -> Option<Indicator> {
    aggregate(newest_first.iter().map(|r| Indicator::of(r)))
}

fn newest_first(records: &[(Option<String>, Record)]) -> Vec<&Record> {
    records.iter().rev().map(|(_, r)| r).collect()
}

/// The glyph for `i`; `frame` turns the spinner.
pub fn glyph(i: Indicator, frame: u64) -> String {
    match i {
        Indicator::Working { progress: None } => SPINNER[frame as usize % SPINNER.len()].into(),
        Indicator::Working { progress: Some(p) } => format!("{} {p}%", SPINNER[frame as usize % SPINNER.len()]),
        Indicator::Blocked(Some(Kind::Permission)) => "⚠".into(),
        Indicator::Blocked(Some(Kind::Question)) => "?".into(),
        Indicator::Blocked(Some(Kind::Auth)) => "⚿".into(),
        Indicator::Blocked(None) => "!".into(),
        Indicator::Done => "✓".into(),
        Indicator::Error => "✗".into(),
    }
}

/// The title and msg of the record behind the pane's indicator, cut short and bidi-isolated (the
/// store already removed controls and invisible characters), or None when it has neither.
pub fn caption(records: &[(Option<String>, Record)]) -> Option<String> {
    caption_of(&newest_first(records))
}

/// `caption` from records most recently reported first.
pub fn caption_of(newest_first: &[&Record]) -> Option<String> {
    let top = indicator_of(newest_first)?;
    let r = newest_first.iter().find(|r| Indicator::of(r).is_some_and(|i| i.rank() == top.rank()))?;
    let text = match (&r.title, &r.msg) {
        (Some(t), Some(m)) => format!("{t}: {m}"),
        (Some(x), None) | (None, Some(x)) => x.clone(),
        (None, None) => return None,
    };
    let mut cut: String = text.chars().take(CAPTION).collect();
    if cut.len() < text.len() {
        cut.pop();
        cut.push('…');
    }
    Some(format!("\u{2068}{cut}\u{2069}"))
}
