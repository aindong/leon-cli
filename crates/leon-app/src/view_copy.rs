//! Copy mode (⌘⇧C: tmux's copy mode with vi keys) and hints (⌘⇧H: tmux-fingers) on the target
//! pane: their keys, what they write over the pane, and what they copy or type. Copy mode moves
//! over the scrollback the view already fetches, selects with the server's selections and
//! searches with its find (docs/architecture/protocol-and-clients.md).

use gpui::ClipboardItem;
use leon_core::hints::{self, Hint};
use leon_core::search::{Kind, Pos, Range};

use super::scroll::Sel;
use super::*;
use crate::copy_mode::{self, Area, CopyMode, Op};
use crate::scrollback::{self, Composed};
use crate::stamp::stamp;

/// Hints on one pane: the labelled tokens of its view and the letters typed so far, as typed (one
/// in upper case types the text into the pane instead of copying it).
pub(super) struct Hints {
    pub(super) block: String,
    hints: Vec<Hint>,
    typed: String,
}

impl Hints {
    /// The labels still showing: those that start with what was typed.
    fn showing(&self) -> impl Iterator<Item = &Hint> {
        let typed = self.typed.to_ascii_lowercase();
        self.hints.iter().filter(move |x| x.label.starts_with(&typed))
    }
}

impl Workspace {
    /// `client.copy_mode` and `client.hints`, on the target pane; either ends the other.
    pub(super) fn mode_action(&mut self, name: &str, cx: &mut Context<Self>) {
        let Some((_, id, _)) = self.target() else { return };
        if let Some(old) = self.term.copy.as_ref().map(|c| c.block.clone()) {
            self.exit_copy(&old);
        }
        self.term.hints = None;
        match name {
            "client.copy_mode" => self.enter_copy(id),
            _ => self.open_hints(id),
        }
        cx.notify();
    }

    /// A key while copy mode or hints are on: true when it was theirs (all but ⌘ chords). A mode
    /// whose pane is no longer the target has ended.
    pub(super) fn mode_key(&mut self, ks: &gpui::Keystroke, chord: Option<&Chord>, cx: &mut Context<Self>) -> bool {
        let target = self.target_block();
        if let Some(old) = self.term.copy.as_ref().map(|c| c.block.clone()).filter(|b| Some(b) != target.as_ref()) {
            self.exit_copy(&old);
        }
        if self.term.hints.as_ref().is_some_and(|h| Some(&h.block) != target.as_ref()) {
            self.term.hints = None;
        }
        if let Some(id) = self.term.copy.as_ref().map(|c| c.block.clone()) {
            self.check_copy(&id);
        }
        if ks.modifiers.platform || (self.term.copy.is_none() && self.term.hints.is_none()) {
            return false;
        }
        match self.term.copy.is_some() {
            true => self.copy_key(ks, chord, cx),
            false => self.hint_key(ks, cx),
        }
        cx.notify();
        true
    }

    /// Whether copy mode's lines on pane `id` are gone: an alt-screen switch moves `scrolled` past
    /// every line kept, and a full scrollback drops its oldest. (Lines past the screen's last
    /// are blank rows a shorter screen dropped: `check_copy` moves onto the last line.)
    fn copy_lost(&self, c: &CopyMode) -> bool {
        let (oldest, _) = scrollback::bounds(&self.live(&c.block));
        c.cursor.line < oldest || c.selection.is_some_and(|(_, a)| a.line < oldest)
    }

    /// Copy mode on pane `id` ends when its lines are gone; a new size keeps them, so it stays,
    /// in the new width and on the new screen's lines.
    pub(super) fn check_copy(&mut self, id: &str) {
        let Some(c) = self.term.copy.as_ref().filter(|c| c.block == id) else { return };
        if self.copy_lost(c) {
            return self.exit_copy(id);
        }
        let live = self.live(id);
        let last = live.view.scrolled + live.lines.saturating_sub(1);
        if let Some(c) = self.term.copy.as_mut() {
            c.cursor.col = c.cursor.col.min(live.cols.saturating_sub(1));
            c.cursor.line = c.cursor.line.min(last);
            if let Some((_, a)) = c.selection.as_mut() {
                a.line = a.line.min(last);
            }
        }
    }

    /// Copy mode starts at the terminal's cursor, or at the view's bottom row when scrolled back;
    /// while it is on, the view doesn't follow the output (`sync_pane` pins it).
    fn enter_copy(&mut self, id: String) {
        let live = self.live(&id);
        let c = self.composed(&id);
        let cursor = match (self.view_top(&id), &live.cursor) {
            (None, Some(k)) => Pos { line: live.view.scrolled + k.line, col: k.col },
            _ => Pos { line: c.top + c.frame.lines.saturating_sub(1), col: 0 },
        };
        if let Some(pv) = self.term.panes.get_mut(&id) {
            pv.selection = None;
        }
        self.term.copy = Some(CopyMode::new(id.clone(), cursor));
        self.sync_pane(&id); // the view holds still from here, as tmux's does
    }

    fn copy_key(&mut self, ks: &gpui::Keystroke, chord: Option<&Chord>, cx: &mut Context<Self>) {
        let Some(cm) = self.term.copy.as_mut() else { return };
        let id = cm.block.clone();
        if let Some((forward, text)) = cm.prompt.as_mut() {
            let m = &ks.modifiers;
            match ks.key.as_str() {
                "escape" => cm.prompt = None,
                "enter" => {
                    // an empty search repeats the last one, the new way (vi's `/` Enter)
                    let (forward, text) = (*forward, std::mem::take(text));
                    cm.search = match (text.is_empty(), cm.search.take()) {
                        (true, Some((_, last))) => Some((forward, last)),
                        _ => Some((forward, text)),
                    };
                    cm.prompt = None;
                    self.copy_search(&id, false);
                }
                "backspace" => {
                    text.pop();
                }
                "space" => text.push(' '),
                _ => {
                    if let Some(typed) = ks.key_char.as_deref().filter(|_| !m.control && !m.alt) {
                        text.extend(typed.chars().filter(|c| !c.is_control()));
                    }
                }
            }
            return;
        }
        let Some(op) = chord.and_then(copy_mode::op) else { return };
        let at = cm.cursor;
        match op {
            Op::Move(m) => {
                let area = self.copy_area(&id);
                let to = copy_mode::motion(at, m, &area, &mut |line| self.line_cells(&id, line));
                self.copy_to(&id, to);
            }
            Op::Select(kind) => {
                cm.select(kind);
                self.copy_to(&id, at);
            }
            Op::Search { forward } => cm.prompt = Some((forward, String::new())),
            Op::Next { reverse } => self.copy_search(&id, reverse),
            Op::Yank => {
                self.copy(&id, cx);
                self.exit_copy(&id);
            }
            Op::Exit => self.exit_copy(&id),
        }
    }

    fn exit_copy(&mut self, id: &str) {
        self.term.copy = None;
        if let Some(pv) = self.term.panes.get_mut(id) {
            pv.selection = None;
        }
        self.scroll_to(id, None);
    }

    /// The lines copy mode moves over: the oldest kept to the screen's last.
    fn copy_area(&self, id: &str) -> Area {
        let live = self.live(id);
        let (oldest, _) = scrollback::bounds(&live);
        Area { oldest, last: live.view.scrolled + live.lines.saturating_sub(1), cols: live.cols, rows: live.lines }
    }

    /// Line `line`'s cells, from the screen or the scrollback (fetched when not cached).
    fn line_cells(&mut self, id: &str, line: usize) -> Vec<char> {
        let live = self.live(id);
        let spans = match line.checked_sub(live.view.scrolled) {
            Some(l) => live.rows.get(l).cloned().unwrap_or_default(),
            None => self.history_row(id, line).map(|r| r.spans).unwrap_or_default(),
        };
        scrollback::cells(&spans, live.cols)
    }

    /// Put the cursor at `to`: the view scrolls to keep it on screen, and a selection follows it.
    fn copy_to(&mut self, id: &str, to: Pos) {
        let Some(cm) = self.term.copy.as_mut() else { return };
        cm.cursor = to;
        let selection = cm.selection;
        let c = self.composed(id);
        let rows = c.frame.lines.max(1);
        if to.line < c.top {
            self.scroll_to(id, Some(to.line));
        } else if to.line >= c.top + rows {
            self.scroll_to(id, Some(to.line + 1 - rows));
        }
        // the server's selection takes both cells, so `v` alone selects the one under the cursor
        let selection = selection.map(|(kind, anchor)| Sel { kind, anchor, head: to, range: None, dragging: false });
        let selecting = selection.is_some();
        self.term.panes.entry(id.to_string()).or_default().selection = selection;
        if selecting {
            self.ask_range(id);
        }
    }

    /// The last search again from the cursor (`reverse`: the other way), over find's matches.
    fn copy_search(&mut self, id: &str, reverse: bool) {
        let Some(cm) = self.term.copy.as_ref() else { return };
        let (Some((forward, query)), at) = (cm.search.clone(), cm.cursor) else { return };
        if query.is_empty() {
            return;
        }
        let args = json!({ "query": query, "regex": false, "case_sensitive": false });
        let block = id.to_string();
        self.term_call_latest("copy find", id, "find", args, move |ws, got| {
            let Ok(found) = got else { return };
            let matches: Vec<Range> = serde_json::from_value(found["matches"].clone()).unwrap_or_default();
            // still in copy mode on that pane, and still where the search started
            let Some(cm) = ws.term.copy.as_mut().filter(|c| c.block == block && c.cursor == at) else { return };
            cm.truncated = (found["truncated"] == true).then_some(matches.len());
            if let Some(to) = copy_mode::pick(&matches, at, forward != reverse) {
                ws.copy_to(&block, to);
            }
        });
    }

    pub(super) fn open_hints(&mut self, id: String) {
        let c = self.composed(&id);
        let rows: Vec<String> =
            c.frame.rows.iter().map(|r| scrollback::cells(r, c.frame.cols).into_iter().collect()).collect();
        // the config's patterns were checked when it loaded
        let patterns = self.keymap.hint_patterns.iter().take(hints::MAX_PATTERNS);
        let extra: Vec<_> = patterns.filter_map(|p| hints::compile(p).ok()).collect();
        let found = hints::find(&rows, &extra);
        if !found.is_empty() {
            self.term.hints = Some(Hints { block: id, hints: found, typed: String::new() });
        }
    }

    /// A letter narrows the labels (one no label continues is ignored); a whole label copies its
    /// text, or types it into the pane when a letter of it came in upper case. Esc leaves.
    fn hint_key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Self>) {
        let Some(h) = self.term.hints.as_mut() else { return };
        let m = &ks.modifiers;
        let letter = ks.key.chars().next().filter(|c| ks.key.len() == 1 && c.is_ascii_alphabetic());
        if ks.key == "escape" {
            self.term.hints = None;
            return;
        }
        if ks.key == "backspace" {
            h.typed.pop();
            return;
        }
        let Some(letter) = letter.filter(|_| !m.control && !m.alt) else { return };
        let letter = if m.shift { letter.to_ascii_uppercase() } else { letter };
        h.typed.push(letter);
        if h.showing().next().is_none() {
            h.typed.pop();
            return;
        }
        let typed = h.typed.to_ascii_lowercase();
        let Some(text) = h.hints.iter().find(|x| x.label == typed).map(|x| x.text.clone()) else { return };
        let (id, upper) = (h.block.clone(), h.typed.chars().any(|c| c.is_ascii_uppercase()));
        self.term.hints = None;
        match upper {
            true => self.block_event(&id, json!({ "type": "paste", "data": text })),
            false => cx.write_to_clipboard(ClipboardItem::new_string(text)),
        }
    }

    /// What pane `id` draws: its view, with copy mode's badge or search prompt (top right, the
    /// program's cursor hidden) or the hint labels written over it.
    pub(super) fn drawn(&self, id: &str) -> Composed {
        let mut c = self.composed(id);
        let t = &DARK;
        if let Some(cm) = self.term.copy.as_ref().filter(|m| m.block == id) {
            c.frame.cursor = None;
            let badge = match (&cm.prompt, cm.selection) {
                (Some((true, q)), _) => format!("/{q}▏"),
                (Some((false, q)), _) => format!("?{q}▏"),
                (None, Some((Kind::Line, _))) => "COPY LINE".into(),
                (None, Some(_)) => "COPY VISUAL".into(),
                (None, None) => "COPY".into(),
            };
            // find stopped short: the matches past these aren't stepped to
            let badge = match cm.truncated.filter(|_| cm.prompt.is_none()) {
                Some(n) => format!("{badge} · {n}+ matches"),
                None => badge,
            };
            let col = c.frame.cols.saturating_sub(badge.chars().count());
            stamp(&mut c.frame, 0, col, &badge, t.bg, t.accent);
        }
        if let Some(h) = self.term.hints.as_ref().filter(|h| h.block == id) {
            for x in h.showing() {
                stamp(&mut c.frame, x.row, x.col, &x.label, t.bg, t.accent);
            }
        }
        c
    }

    /// What copy mode and hints mark on pane `id`'s rows: the cursor (outlined), and each hint's
    /// text after its label.
    pub(super) fn mode_marks(&self, id: &str, c: &Composed) -> Vec<CellMark> {
        let mut out = Vec::new();
        if let Some(cm) = self.term.copy.as_ref().filter(|m| m.block == id) {
            if let Some(row) = cm.cursor.line.checked_sub(c.top).filter(|r| *r < c.frame.lines) {
                out.push(CellMark { row, from: cm.cursor.col, to: cm.cursor.col, mark: Mark::Current });
            }
        }
        if let Some(h) = self.term.hints.as_ref().filter(|h| h.block == id) {
            for x in h.showing().filter(|x| x.cells > x.label.len()) {
                let (from, to) = (x.col + x.label.len(), x.col + x.cells - 1);
                out.push(CellMark { row: x.row, from, to, mark: Mark::Found });
            }
        }
        out
    }

    /// Copy mode's cursor on pane `id` and its line's text, while copy mode is on there.
    pub fn copy_cursor(&self, id: &str) -> Option<(Pos, String)> {
        let cm = self.term.copy.as_ref().filter(|m| m.block == id && !self.copy_lost(m))?;
        let live = self.live(id);
        let spans = match cm.cursor.line.checked_sub(live.view.scrolled) {
            Some(l) => live.rows.get(l).cloned().unwrap_or_default(),
            None => self.cached_row(id, cm.cursor.line).map(|r| r.spans.clone()).unwrap_or_default(),
        };
        Some((cm.cursor, scrollback::text_of(&spans, live.cols, 0, live.cols.saturating_sub(1))))
    }

    /// Where pane `id` draws copy mode's cursor (outlined): its view row and column.
    pub fn copy_mark(&self, id: &str) -> Option<(usize, usize)> {
        let marks = self.mode_marks(id, &self.composed(id));
        marks.iter().find(|m| m.mark == Mark::Current).map(|m| (m.row, m.from))
    }

    /// The hints showing on pane `id`: (label, text).
    pub fn hint_labels(&self, id: &str) -> Vec<(String, String)> {
        let h = self.term.hints.as_ref().filter(|h| h.block == id);
        h.map(|h| h.hints.iter().map(|x| (x.label.clone(), x.text.clone())).collect()).unwrap_or_default()
    }

    /// The text pane `id` draws now, labels and badges included, a line per row.
    pub fn drawn_text(&self, id: &str) -> Option<String> {
        let shown = self.app.model().screens.contains_key(id); // the model's lock goes before drawn takes it
        shown.then(|| self.drawn(id).frame.text())
    }
}
