//! Text into the focused pane through GPUI's input handler (ADR R9), as Zed's terminal does:
//! `key_down` leaves a key that types text, and macOS hands that text here, or what an input
//! method composes (marked text, drawn at the cursor and never sent) and then commits, or what the
//! emoji picker inserts. Everything else a key sends still goes from `key_down`.

use std::ops::Range;

use gpui::{Entity, EntityInputHandler, InputHandler, UTF16Selection};

use super::*;

/// What the input method is composing on pane `block`, and its selection in it (UTF-16).
pub(super) struct Marked {
    block: String,
    text: String,
    selected: Range<usize>,
}

#[derive(Default)]
pub(super) struct Ime {
    marked: Option<Marked>,
    /// Marked text ended unsent (its pane lost the focus or stopped being the target) while the
    /// input method may still hold it: its next commit is dropped, unless it marks text again.
    stale: bool,
    /// The text of the key `key_down` just left to the handler: text equal to it is that key typed,
    /// anything else (Space committing 你好, a commit with no key) is a composition's.
    key: Option<String>,
    commits: u64,
}

/// leon's input handler: GPUI's `ElementInputHandler`, but with press-and-hold off, so a held key
/// repeats in the terminal instead of opening macOS's accent picker.
pub struct PaneInput(Entity<Workspace>);

impl PaneInput {
    /// The handler the workspace registers for its focused pane (and tests drive).
    pub fn new(ws: Entity<Workspace>) -> Self {
        PaneInput(ws)
    }
}

impl InputHandler for PaneInput {
    fn selected_text_range(&mut self, ignore: bool, window: &mut Window, cx: &mut App) -> Option<UTF16Selection> {
        self.0.update(cx, |ws, cx| ws.selected_text_range(ignore, window, cx))
    }
    fn marked_text_range(&mut self, window: &mut Window, cx: &mut App) -> Option<Range<usize>> {
        self.0.update(cx, |ws, cx| ws.marked_text_range(window, cx))
    }
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<String> {
        self.0.update(cx, |ws, cx| ws.text_for_range(range, adjusted, window, cx))
    }
    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, window: &mut Window, cx: &mut App) {
        self.0.update(cx, |ws, cx| ws.replace_text_in_range(range, text, window, cx))
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.0.update(cx, |ws, cx| ws.replace_and_mark_text_in_range(range, text, selected, window, cx))
    }
    fn unmark_text(&mut self, window: &mut Window, cx: &mut App) {
        self.0.update(cx, |ws, cx| ws.unmark_text(window, cx))
    }
    fn bounds_for_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut App) -> Option<Bounds<Pixels>> {
        self.0.update(cx, |ws, cx| ws.bounds_for_range(range, Bounds::default(), window, cx))
    }
    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<usize> {
        self.0.update(cx, |ws, cx| ws.character_index_for_point(point, window, cx))
    }
    fn apple_press_and_hold_enabled(&mut self) -> bool {
        false
    }
}

/// The input method sees the marked text as the whole document, with the selection it set.
impl EntityInputHandler for Workspace {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        // only a range inside the marked text, never adjusted: GPUI writes an adjusted range through
        // AppKit's `actualRange`, which may be null (platform/mac/window.rs, attributedSubstring…)
        let _ = adjusted;
        let utf16: Vec<u16> = self.ime.marked.as_ref()?.text.encode_utf16().collect();
        String::from_utf16(utf16.get(range)?).ok()
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let range = self.ime.marked.as_ref().map_or(0..0, |m| m.selected.clone());
        Some(UTF16Selection { range, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.ime.marked.as_ref().map(|m| 0..m.text.encode_utf16().count())
    }

    /// AppKit: unmarking accepts the marked text as if it had been inserted.
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(m) = self.ime.marked.take() else { return };
        cx.notify();
        if let Some((sid, id, _)) = self.target().filter(|(_, id, _)| *id == m.block) {
            self.commit(&sid, &id, &m.text, cx);
        }
    }

    /// Typed text, an input method's commit, the emoji picker's pick: to the pane. A range
    /// replaces that part of the marked text, and the rest of it goes with the commit.
    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let marked = self.ime.marked.take();
        let key = self.ime.key.take();
        if marked.is_some() {
            cx.notify();
        } else if self.ime.stale && key.as_deref() != Some(text) {
            self.ime.stale = false;
            return; // the commit of a composition no longer shown (a typed key doesn't end it)
        }
        let Some((sid, id, _)) = self.target() else { return };
        let text = match (marked, range) {
            (Some(m), Some(r)) => splice(&m.text, r, text),
            _ => text.to_string(),
        };
        self.commit(&sid, &id, &text, cx);
    }

    /// A range replaces that part of the marked text; the selection is within the new text.
    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((_, block, _)) = self.target() else { return };
        let inserted = text.encode_utf16().count();
        let old = self.ime.marked.as_ref().filter(|m| m.block == block).map(|m| m.text.as_str());
        // `at`: where the inserted text starts in the marked text
        let (text, at) = match (old, range) {
            (Some(old), Some(r)) => (splice(old, r.clone(), text), r.start.min(old.encode_utf16().count())),
            _ => (text.to_string(), 0),
        };
        let end = text.encode_utf16().count();
        let selected = selected.map_or(at + inserted..at + inserted, |s| at + s.start..at + s.end);
        let selected = selected.start.min(end)..selected.end.min(end);
        if self.view_top(&block).is_some() {
            self.scroll_to(&block, None); // composing goes back to the prompt, as typing does
        }
        (self.ime.stale, self.ime.key) = (false, None);
        self.ime.marked = (!text.is_empty()).then_some(Marked { block, text, selected });
        cx.notify();
    }

    /// Where the candidate window goes: the anchor, a cell further per UTF-16 unit (a wide
    /// character counts one: the window sits a little left of its end).
    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let b = self.ime_anchor()?;
        Some(Bounds { origin: b.origin + gpui::point(b.size.width * range.start as f32, px(0.)), ..b })
    }

    fn character_index_for_point(
        &mut self,
        _: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}

impl Workspace {
    /// Text from the input handler into pane `id`: none while copy mode or hints are on there, and
    /// no control character (an input method commits none; a held ⌘↩'s newline would run the line).
    fn commit(&mut self, sid: &str, id: &str, text: &str, cx: &mut Context<Self>) {
        let text: String = text.chars().filter(|c| !c.is_control()).collect();
        if text.is_empty() || self.pane_mode(id) {
            return;
        }
        self.ime.commits += 1;
        self.write_pane(sid, id, text.as_bytes(), cx);
    }

    /// `key_down` leaves this key's text to the handler.
    pub(super) fn leave_key(&mut self, text: Option<&str>) {
        self.ime.key = text.map(String::from);
    }

    /// Copy mode or hints are on in pane `id`: they take its keys, and no text.
    pub(super) fn pane_mode(&self, id: &str) -> bool {
        self.term.copy.as_ref().is_some_and(|c| c.block == id)
            || self.term.hints.as_ref().is_some_and(|h| h.block == id)
    }

    /// Bytes for pane `id`, from a key or the input handler, in the window's one call order.
    pub(super) fn write_pane(&mut self, sid: &str, id: &str, bytes: &[u8], cx: &mut Context<Self>) {
        // only what reaches the pane is timed (LEON_FRAME_LOG): its echo moves the cursor
        if self.stats.borrow().logging() {
            if let Some(stamped) = crate::input::current_event_stamp() {
                let cursor = self.drawn(id).frame.cursor.map(|c| (c.line, c.col));
                self.stats.borrow_mut().key(stamped, id.to_string(), cursor);
            }
        }
        self.app.write(sid, id, bytes);
        self.key_sent(id);
        if self.view_top(id).is_some() {
            self.scroll_to(id, None); // typing goes back to the prompt
            cx.notify();
        }
    }

    /// Where marked text is drawn: the target pane's cursor cell, or its bottom-left cell while the
    /// program hides the cursor (the frame then has none: the input method still has a place).
    pub fn ime_anchor(&self) -> Option<Bounds<Pixels>> {
        let (_, id, _) = self.target()?;
        let (frame, _, _) = self.grid_given(&id)?;
        let p = self.term.painted.borrow().get(&id).copied()?;
        let (line, col) = frame.cursor.as_ref().map_or((p.size.0.saturating_sub(1), 0), |c| (c.line, c.col));
        let origin = p.origin + gpui::point(p.cell.0 * col as f32, p.cell.1 * line as f32);
        Some(Bounds { origin, size: gpui::size(p.cell.0, p.cell.1) })
    }

    /// What the input method is composing (tests).
    pub fn marked_text(&self) -> Option<String> {
        self.ime.marked.as_ref().map(|m| m.text.clone())
    }

    /// Texts the input handler wrote to a pane (tests).
    pub fn input_commits(&self) -> u64 {
        self.ime.commits
    }

    /// Marked text ends, unsent, once its pane hasn't the focus or isn't the target any more.
    pub(super) fn end_stale_marked(&mut self, window: &Window) {
        let target = self.target().map(|(_, id, _)| id);
        let gone = |m: &Marked| !self.focus.is_focused(window) || Some(&m.block) != target.as_ref();
        if self.ime.marked.as_ref().is_some_and(gone) {
            self.ime.marked = None;
            self.ime.stale = true;
        }
    }

    /// Registers leon's input handler while painting: GPUI takes it for the frame when the
    /// workspace's handle has the focus (the pane), and hands it the window's key text.
    pub(super) fn input_canvas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (focus, ws) = (self.focus.clone(), cx.entity());
        canvas(|_, _, _| {}, move |_, _, window, cx| window.handle_input(&focus, PaneInput::new(ws), cx))
            .absolute()
            .size_full()
    }

    /// The marked text over the target pane's cursor, underlined, in the pane's font.
    pub(super) fn render_marked(&self) -> Option<gpui::AnyElement> {
        let m = self.ime.marked.as_ref()?;
        let at = self.ime_anchor().filter(|_| self.target().is_some_and(|(_, id, _)| id == m.block))?;
        let t = self.pane_theme(&m.block);
        let text = div()
            .debug_selector(|| sel("marked-text".into()))
            .absolute()
            .left(at.origin.x)
            .top(at.origin.y)
            .h(at.size.height)
            .bg(rgb(t.bg))
            .text_color(rgb(t.fg))
            .font_family(self.font_family.clone())
            .text_size(px(self.font_size))
            .line_height(at.size.height)
            .whitespace_nowrap()
            .text_decoration_1()
            .text_decoration_solid()
            .text_decoration_color(rgb(t.fg))
            .child(m.text.clone());
        Some(text.into_any_element())
    }
}

/// `old` with its UTF-16 range `r` (clamped to it) replaced by `new`.
fn splice(old: &str, r: Range<usize>, new: &str) -> String {
    let u: Vec<u16> = old.encode_utf16().collect();
    let start = r.start.min(u.len());
    let end = r.end.clamp(start, u.len());
    String::from_utf16_lossy(&u[..start]) + new + &String::from_utf16_lossy(&u[end..])
}
