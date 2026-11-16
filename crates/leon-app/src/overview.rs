//! The overview (`client.overview.toggle`, ⌘⇧\; this session's tabs with ⌘\): every tab as a card,
//! grouped under its session and, once there are hosts, its machine. Which cards, in which
//! groups, where each goes in the grid, and where the arrow keys move. GPUI-free; the window draws
//! it (view_overview.rs).

use std::collections::HashMap;

use crate::palette::{self, Item};
use crate::panes::Rect;
use crate::remote::{Model, Session};
use crate::status::{self, Indicator};
use crate::switcher;

/// The space around the grid and between cards, a label's row, and a card's header.
pub const PAD: f32 = 24.;
pub const GAP: f32 = 16.;
pub const LABEL: f32 = 28.;
pub const HEADER: f32 = 28.;

/// One machine as the overview reads it: its last model (None if it was never reached) and why it
/// isn't connected now (`HostState::note`).
pub struct Machine<'a> {
    pub host: usize,
    pub label: &'a str,
    pub model: Option<&'a Model>,
    pub note: Option<String>,
}

/// Every tab, or the tabs of one session.
#[derive(Debug, Clone, PartialEq)]
pub enum Scope {
    All,
    Session { host: usize, session_id: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub host: usize,
    pub session_id: String,
    pub window_id: String,
    /// The sidebar's title for the tab.
    pub title: String,
    pub status: Option<Indicator>,
    /// Its machine isn't connected: drawn grey, from the last model.
    pub greyed: bool,
}

/// One session's cards. The first group of a machine carries its label (when hosts are shown) and
/// why it isn't connected; a machine never reached is one group with no session and no cards.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub host: usize,
    pub host_label: Option<String>,
    pub note: Option<String>,
    pub session: Option<String>,
    pub cards: Vec<Card>,
}

/// Each tab's status as the sidebar shows it: the most pressing of its blocks'.
pub fn tab_statuses(s: &Session) -> HashMap<&str, Option<Indicator>> {
    // each block read once: per window per block would be a million a poll at the caps
    let mut by_window = HashMap::<&str, Option<Indicator>>::new();
    for b in &s.blocks {
        let shown = status::indicator_of(&b.newest_first());
        let w = by_window.entry(&b.window_id).or_default();
        *w = status::aggregate([*w, shown]);
    }
    by_window
}

/// The groups for `scope`, machines in order and sessions in each machine's order: the sidebar's.
pub fn groups(machines: &[Machine], scope: &Scope, hosts: bool) -> Vec<Group> {
    let mut out = Vec::new();
    for m in machines {
        if let Scope::Session { host, .. } = scope {
            if *host != m.host {
                continue;
            }
        }
        let first = out.len();
        for (i, s) in m.model.map(|m| m.sessions.as_slice()).unwrap_or_default().iter().enumerate() {
            if matches!(scope, Scope::Session { session_id, .. } if *session_id != s.id) {
                continue;
            }
            let statuses = tab_statuses(s);
            let cards = s
                .windows
                .iter()
                .map(|w| Card {
                    host: m.host,
                    session_id: s.id.clone(),
                    window_id: w.id.clone(),
                    title: switcher::tab_title(s, w),
                    status: statuses.get(w.id.as_str()).copied().flatten(),
                    greyed: m.note.is_some(),
                })
                .collect();
            out.push(Group {
                host: m.host,
                host_label: None,
                note: None,
                session: Some(switcher::session_title(s, i)),
                cards,
            });
        }
        if out.len() == first && (m.model.is_none() && m.note.is_some()) && *scope == Scope::All {
            out.push(Group { host: m.host, host_label: None, note: None, session: None, cards: vec![] });
        }
        if let Some(g) = out.get_mut(first) {
            g.host_label = hosts.then(|| m.label.to_string());
            g.note = m.note.clone();
        }
    }
    out
}

/// The groups with only the cards whose title matches `query` (the palette's filter), in their
/// order; a session left with none drops out, a machine's label stays with its first group left.
pub fn filter(groups: &[Group], query: &str) -> Vec<Group> {
    if query.trim().is_empty() {
        return groups.to_vec();
    }
    let mut out: Vec<Group> = Vec::new();
    let mut carried: Option<(usize, Option<String>, Option<String>)> = None;
    for g in groups {
        if g.host_label.is_some() || g.note.is_some() {
            carried = Some((g.host, g.host_label.clone(), g.note.clone()));
        }
        let items: Vec<Item> = g
            .cards
            .iter()
            .map(|c| Item {
                name: c.title.clone(),
                title: c.title.clone(),
                keywords: vec![],
                shortcut: None,
                args: None,
            })
            .collect();
        let mut keep = palette::filter(&items, query);
        keep.sort_unstable();
        if keep.is_empty() {
            continue;
        }
        let mut g = Group { cards: keep.into_iter().map(|i| g.cards[i].clone()).collect(), ..g.clone() };
        if let Some((_, label, note)) = carried.take_if(|c| c.0 == g.host) {
            (g.host_label, g.note) = (label, note);
        }
        out.push(g);
    }
    out
}

/// The cards in group order, as the grid lists them.
pub fn cards(groups: &[Group]) -> Vec<Card> {
    groups.iter().flat_map(|g| g.cards.iter().cloned()).collect()
}

/// Where everything goes, in the grid's own space (x from its left, y from its top).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// Each card, in `cards` order.
    pub cards: Vec<Rect>,
    /// Each label row: its rect, the group it heads, and whether it is the machine's label.
    pub labels: Vec<(Rect, usize, bool)>,
    pub height: f32,
    pub cols: usize,
}

/// The card widths `fit` tries, widest first.
const CARD_WS: (f32, f32) = (640., 240.);

/// The grid at the widest card that lets everything fit `height` (the window less its titlebar),
/// down to `CARD_WS.1`, below which it scrolls: a few tabs fill the window, many scroll.
pub fn fit(groups: &[Group], width: f32, height: f32, aspect: f32) -> Layout {
    let mut card = CARD_WS.0;
    loop {
        let l = layout_at(groups, width, aspect, card);
        if l.height <= height || card <= CARD_WS.1 {
            return l;
        }
        card -= 20.;
    }
}

/// Lay the groups out `width` wide: as many columns as fit at about `card` px, each card a header
/// over a thumbnail `aspect` (height / width) as tall as it is wide. A session's cards are a block
/// under its label, as wide as its cards (at most every column); blocks sit side by side while
/// they fit, and a machine's label starts a new row.
pub fn layout_at(groups: &[Group], width: f32, aspect: f32, card: f32) -> Layout {
    let inner = (width - 2. * PAD).max(1.);
    let cols = (((inner + GAP) / (card + GAP)).floor() as usize).max(1);
    let w = ((inner - GAP * (cols - 1) as f32) / cols as f32).max(1.);
    let h = HEADER + w * aspect;
    let mut l = Layout { cols, ..Layout::default() };
    // the row of blocks being filled: its top, the next free column, and its tallest block
    let (mut y, mut col, mut band) = (PAD, 0, 0.);
    for (gi, g) in groups.iter().enumerate() {
        let span = g.cards.len().clamp(1, cols);
        if col > 0 && (g.host_label.is_some() || col + span > cols) {
            (y, col, band) = (y + band, 0, 0.);
        }
        if g.host_label.is_some() {
            l.labels.push((Rect { x: PAD, y, w: inner, h: LABEL }, gi, true));
            y += LABEL;
        }
        let x = PAD + col as f32 * (w + GAP);
        let mut top = y;
        if g.session.is_some() {
            l.labels.push((Rect { x, y: top, w: span as f32 * (w + GAP) - GAP, h: LABEL }, gi, false));
            top += LABEL;
        }
        for (i, _) in g.cards.iter().enumerate() {
            let (row, c) = (i / cols, i % cols);
            l.cards.push(Rect { x: x + c as f32 * (w + GAP), y: top + row as f32 * (h + GAP), w, h });
        }
        let rows = g.cards.len().div_ceil(cols);
        band = f32::max(band, top - y + rows as f32 * (h + GAP) + if rows == 0 { GAP } else { 0. });
        col += span;
        if col >= cols {
            (y, col, band) = (y + band, 0, 0.);
        }
    }
    l.height = y + band + PAD - GAP;
    l
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Step {
    Left,
    Right,
    Up,
    Down,
}

/// The card an arrow key moves to from `from`: along the order for ← and →, else the card in the
/// next row up or down whose centre is nearest across. Stays put at an edge.
pub fn step(cards: &[Rect], from: usize, step: Step) -> usize {
    let Some(at) = cards.get(from) else { return from };
    let mid = |r: &Rect| r.x + r.w / 2.;
    match step {
        Step::Left => from.saturating_sub(1),
        Step::Right => (from + 1).min(cards.len().saturating_sub(1)),
        Step::Up | Step::Down => {
            let down = step == Step::Down;
            let beyond = |r: &Rect| if down { r.y > at.y + 0.5 } else { r.y < at.y - 0.5 };
            let row =
                cards.iter().filter(|r| beyond(r)).map(|r| r.y).reduce(|a, b| if down { a.min(b) } else { a.max(b) });
            let Some(row) = row else { return from };
            (0..cards.len())
                .filter(|&i| (cards[i].y - row).abs() < 0.5)
                .min_by(|&a, &b| (mid(&cards[a]) - mid(at)).abs().total_cmp(&(mid(&cards[b]) - mid(at)).abs()))
                .unwrap_or(from)
        }
    }
}
