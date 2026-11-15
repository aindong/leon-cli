//! What the app shows for OSC 7501 program status (docs/agents.md, "Program status"): one indicator per
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
            Indicator::Working { .. } => t.working,
            Indicator::Blocked(_) => t.blocked,
            Indicator::Done => t.done,
            Indicator::Error => t.error,
        }
    }
}

/// The state in a word, for where the program gave no caption.
pub fn word(i: Indicator) -> &'static str {
    match i {
        Indicator::Working { .. } => "working",
        Indicator::Blocked(_) => "needs you",
        Indicator::Done => "done",
        Indicator::Error => "error",
    }
}

/// The highest-priority indicator; on a tie the first.
pub fn aggregate(all: impl IntoIterator<Item = Option<Indicator>>) -> Option<Indicator> {
    all.into_iter().flatten().fold(None, |best, i| match best {
        Some(b) if b.rank() >= i.rank() => Some(b),
        _ => Some(i),
    })
}

/// One terminal's indicator from its records, most recently reported first.
pub fn indicator_of(newest_first: &[&Record]) -> Option<Indicator> {
    aggregate(newest_first.iter().map(|r| Indicator::of(r)))
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

/// The title and msg of the record behind the pane's indicator (records most recently reported
/// first), cut short and bidi-isolated (the store already removed controls and invisible
/// characters), or None when it has neither.
pub fn caption_of(newest_first: &[&Record]) -> Option<String> {
    let r = top_record(newest_first)?;
    let text = match (&r.title, &r.msg) {
        (Some(t), Some(m)) => format!("{t}: {m}"),
        (Some(x), None) | (None, Some(x)) => x.clone(),
        (None, None) => return None,
    };
    Some(isolated(&text))
}

/// The title of the record behind the indicator: what an agent calls its task.
pub fn status_title(newest_first: &[&Record]) -> Option<String> {
    top_record(newest_first)?.title.as_deref().map(|t| t.chars().take(CAPTION).collect())
}

/// The caption's msg alone, for where its title is said already.
pub fn status_msg(newest_first: &[&Record]) -> Option<String> {
    top_record(newest_first)?.msg.as_deref().map(isolated)
}

/// The newest record in the highest-priority state.
fn top_record<'a>(newest_first: &[&'a Record]) -> Option<&'a Record> {
    let top = indicator_of(newest_first)?;
    newest_first.iter().copied().find(|r| Indicator::of(r).is_some_and(|i| i.rank() == top.rank()))
}

/// Cut short and bidi-isolated.
fn isolated(text: &str) -> String {
    let mut cut: String = text.chars().take(CAPTION).collect();
    if cut.len() < text.len() {
        cut.pop();
        cut.push('…');
    }
    format!("\u{2068}{cut}\u{2069}")
}
