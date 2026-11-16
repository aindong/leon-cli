//! The overview's keys: arrows move, ↩ goes, esc goes back, its own shortcuts close it or switch
//! between this session and every tab, and the rest filters.

use super::*;

impl Workspace {
    /// A key while the overview is open: arrows move, ↩ goes, esc goes back, the rest filters. Its
    /// own keys close it; other bindings wait until it is closed.
    pub(in crate::view) fn overview_key(
        &mut self,
        ks: &gpui::Keystroke,
        chord: Option<&Chord>,
        cx: &mut Context<Self>,
    ) {
        let m = &ks.modifiers;
        let named = |name: &str| ks.key == name && !m.platform && !m.control;
        let bound = chord.and_then(|c| match self.keymap.press(&[], c) {
            Pressed::Run(b) => b.action,
            _ => None,
        });
        if let Some(a) =
            bound.as_deref().filter(|a| matches!(*a, "client.overview.toggle" | "client.overview.session.toggle"))
        {
            let scope = self.overview_scope(a == "client.overview.toggle");
            if self.overview.open.as_ref().is_some_and(|o| o.scope == scope) {
                self.close_overview(false, cx);
            } else if let Some(o) = self.overview.open.as_mut() {
                (o.scope, o.selected) = (scope, 0);
                self.overview.scroll.set_offset(point(px(0.), px(0.)));
                cx.notify();
            }
            return;
        }
        // tabs can close (or a host's model shrink) under the overview: stay on a card
        let n = self.overview_cards().len();
        if let Some(o) = self.overview.open.as_mut() {
            o.selected = o.selected.min(n.saturating_sub(1));
        }
        let steps = [("left", Step::Left), ("right", Step::Right), ("up", Step::Up), ("down", Step::Down)];
        if named("escape") {
            self.close_overview(false, cx);
        } else if named("enter") {
            if !self.overview_cards().is_empty() {
                self.close_overview(true, cx);
            }
        } else if let Some((_, step)) = steps.iter().find(|(k, _)| named(k)) {
            let groups = self.overview.open.as_ref().map(|o| self.overview_groups(o)).unwrap_or_default();
            let rects = self.overview_layout(&groups).cards;
            if let Some(o) = self.overview.open.as_mut() {
                o.selected = overview::step(&rects, o.selected, *step);
                let r = rects.get(o.selected).copied();
                self.reveal(r);
            }
        } else if let Some(o) = self.overview.open.as_mut() {
            if named("backspace") {
                o.query.pop();
            } else if let Some(typed) =
                ks.key_char.as_deref().filter(|t| !m.platform && !m.control && t.chars().any(|c| !c.is_control()))
            {
                o.query.extend(typed.chars().filter(|c| !c.is_control()));
            } else if ks.key == "space" {
                o.query.push(' ');
            } else {
                return;
            }
            o.selected = 0;
            self.overview.scroll.set_offset(point(px(0.), px(0.)));
        }
        cx.notify();
    }

    /// Scroll the grid so card rect `r` is in view.
    fn reveal(&self, r: Option<panes::Rect>) {
        let Some(r) = r else { return };
        let s = &self.overview.scroll;
        let (top, h) = (-f32::from(s.offset().y), f32::from(s.bounds().size.height));
        if h <= 0. {
            return;
        }
        let y = if r.y < top {
            r.y - overview::PAD
        } else if r.y + r.h > top + h {
            r.y + r.h + overview::PAD - h
        } else {
            return;
        };
        s.set_offset(point(px(0.), px(-y.max(0.))));
    }
}
