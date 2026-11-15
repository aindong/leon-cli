//! Find (cmd+f): a bar in the focused pane's header, matches across the scrollback from the
//! server (`dev.leon.terminal.find`), enter and shift+enter to step, alt+c and alt+r for case and
//! regex, esc to close.

use leon_core::search::Range;

use super::*;

/// While output arrives, find runs again at most this often.
const RERUN: Duration = Duration::from_millis(250);

pub(super) struct Find {
    pub(super) block: String,
    pub(super) query: String,
    pub(super) case: bool,
    pub(super) regex: bool,
    /// Oldest first, by absolute line: they stay put as output arrives.
    pub(super) matches: Vec<Range>,
    pub(super) current: Option<usize>,
    pub(super) error: Option<String>,
    /// The screen update count the matches were found at, and when.
    seen: Option<u64>,
    ran: Instant,
}

impl Workspace {
    pub(super) fn find_action(&mut self, name: &str, cx: &mut Context<Self>) {
        match name {
            "client.find.open" => {
                let Some((_, id, _)) = self.target() else { return };
                if self.term.find.as_ref().is_none_or(|f| f.block != id) {
                    let (case, regex) = self.term.find.as_ref().map_or((false, false), |f| (f.case, f.regex));
                    self.term.find = Some(Find {
                        block: id,
                        query: String::new(),
                        case,
                        regex,
                        matches: Vec::new(),
                        current: None,
                        error: None,
                        seen: None,
                        ran: Instant::now(),
                    });
                }
            }
            "client.find.next" => self.step_find(true),
            _ => self.step_find(false),
        }
        cx.notify();
    }

    /// A key while the find bar is open; false lets it go on to the keymap (cmd chords do).
    pub(super) fn find_key(&mut self, ks: &gpui::Keystroke, cx: &mut Context<Self>) -> bool {
        let m = &ks.modifiers;
        let Some(f) = self.term.find.as_mut().filter(|_| !m.platform && !m.control) else { return false };
        match ks.key.as_str() {
            "escape" => self.term.find = None,
            "enter" => self.step_find(!m.shift),
            "backspace" => {
                f.query.pop();
                self.run_find();
            }
            "c" if m.alt => {
                f.case = !f.case;
                self.run_find();
            }
            "r" if m.alt => {
                f.regex = !f.regex;
                self.run_find();
            }
            "space" => {
                f.query.push(' ');
                self.run_find();
            }
            _ => match ks.key_char.as_deref().filter(|c| !c.is_empty() && !m.alt) {
                Some(typed) => {
                    f.query.extend(typed.chars().filter(|c| !c.is_control()));
                    self.run_find();
                }
                None => return false,
            },
        }
        cx.notify();
        true
    }

    /// Ask the server for the matches; the newest is current at first, else the one that was.
    pub(super) fn run_find(&mut self) {
        let Some(f) = self.term.find.as_ref() else { return };
        let (id, was) = (f.block.clone(), f.current.and_then(|i| f.matches.get(i)).copied());
        let args = json!({ "query": f.query, "regex": f.regex, "case_sensitive": f.case });
        // read first: output arriving during the call brings another run
        let seen = self.app.model().screen_updates.get(&id).copied();
        let got = self.term_call(&id, "find", args);
        let Some(f) = self.term.find.as_mut() else { return };
        (f.seen, f.ran) = (seen, Instant::now());
        match got {
            Ok(v) => {
                f.matches = serde_json::from_value(v["matches"].clone()).unwrap_or_default();
                f.error = None;
            }
            Err(e) => (f.matches, f.error) = (Vec::new(), Some(e.message)),
        }
        let kept = was.and_then(|w| f.matches.iter().position(|m| *m == w));
        f.current = kept.or(f.matches.len().checked_sub(1));
        if kept.is_none() {
            self.show_match();
        }
    }

    /// Whether output arrived since the matches were found, long enough ago to look again.
    pub(super) fn find_due(&self) -> bool {
        self.term.find.as_ref().is_some_and(|f| {
            !f.query.is_empty()
                && f.ran.elapsed() >= RERUN
                && self.app.model().screen_updates.get(&f.block).copied() != f.seen
        })
    }

    fn step_find(&mut self, forward: bool) {
        let Some(f) = self.term.find.as_mut().filter(|f| !f.matches.is_empty()) else { return };
        let n = f.matches.len();
        f.current = Some(match f.current {
            Some(i) if forward => (i + 1) % n,
            Some(i) => (i + n - 1) % n,
            None => n - 1,
        });
        self.show_match();
    }

    /// Scroll so the current match is in view, in the middle when it wasn't.
    fn show_match(&mut self) {
        let Some(f) = self.term.find.as_ref() else { return };
        let Some(m) = f.current.and_then(|i| f.matches.get(i)).copied() else { return };
        let id = f.block.clone();
        let c = self.composed(&id);
        if (c.top..c.top + c.frame.lines).contains(&m.start.line) {
            return;
        }
        let top = m.start.line.saturating_sub(c.frame.lines / 2);
        self.scroll_to(&id, Some(top));
    }

    /// "n of N" while the bar is open: the current match's place and how many there are.
    pub fn find_status(&self) -> Option<(usize, usize)> {
        let f = self.term.find.as_ref()?;
        Some((f.current.map_or(0, |i| i + 1), f.matches.len()))
    }

    /// How many matches pane `id` draws highlighted now.
    pub fn highlighted_matches(&self, id: &str) -> usize {
        let Some(f) = self.term.find.as_ref().filter(|f| f.block == id) else { return 0 };
        let c = self.composed(id);
        let shown = c.top..c.top + c.frame.lines;
        f.matches.iter().filter(|m| m.start.line < shown.end && m.end.line >= shown.start).count()
    }

    /// The find bar, in pane `id`'s header while it is open there.
    pub(super) fn render_find_bar(&self, id: &str, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let f = self.term.find.as_ref().filter(|f| f.block == id)?;
        let (t, theme) = (&self.theme, self.theme);
        let count = match (&f.error, f.matches.len()) {
            (Some(_), _) => "bad pattern".to_string(),
            (None, 0) if f.query.is_empty() => String::new(),
            (None, 0) => "no matches".to_string(),
            (None, n) => format!("{} of {n}", f.current.map_or(0, |i| i + 1)),
        };
        let toggle = |name: &'static str, label: &'static str, on: bool, key: &'static str, cx: &mut Context<Self>| {
            div()
                .id(SharedString::from(format!("find-{name}-{id}")))
                .px(px(5.))
                .rounded(px(4.))
                .font_family(self.font_family.clone())
                .cursor_pointer()
                .text_color(rgb(if on { t.title } else { t.muted }))
                .when(on, |d| d.bg(rgb(t.selected)))
                .hover(|d| d.text_color(rgb(t.title)))
                .child(label)
                .tooltip(move |_, cx| cx.new(|_| super::bars::Tip(format!("{label}   ⌥{key}").into(), theme)).into())
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, _, _, cx| {
                        cx.stop_propagation();
                        if let Some(f) = ws.term.find.as_mut() {
                            match name {
                                "case" => f.case = !f.case,
                                _ => f.regex = !f.regex,
                            }
                        }
                        ws.run_find();
                        cx.notify();
                    }),
                )
        };
        Some(
            div()
                .debug_selector(|| sel(format!("find-bar-{id}")))
                .flex()
                .flex_none()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .w(px(180.))
                        .h(px(20.))
                        .px(px(6.))
                        .flex()
                        .items_center()
                        .rounded(px(4.))
                        .bg(rgb(t.bg))
                        .border_1()
                        .border_color(rgb(t.accent))
                        .overflow_hidden()
                        .text_color(rgb(t.title))
                        .child(format!("{}▏", f.query)),
                )
                .child(div().min_w(px(64.)).text_color(rgb(t.muted)).child(count))
                .child(toggle("case", "Aa", f.case, "C", cx))
                .child(toggle("regex", ".*", f.regex, "R", cx)),
        )
    }
}
