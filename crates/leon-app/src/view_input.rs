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
    /// Texts the handler wrote to a pane (tests).
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
        let utf16: Vec<u16> = self.ime.marked.as_ref()?.text.encode_utf16().collect();
        let range = range.start.min(utf16.len())..range.end.min(utf16.len());
        *adjusted = Some(range.clone());
        Some(String::from_utf16_lossy(&utf16[range]))
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let range = self.ime.marked.as_ref().map_or(0..0, |m| m.selected.clone());
        Some(UTF16Selection { range, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.ime.marked.as_ref().map(|m| 0..m.text.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.ime.marked.take().is_some() {
            cx.notify();
        }
    }

    /// Typed text, an input method's commit, the emoji picker's pick: to the pane.
    fn replace_text_in_range(&mut self, _: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        if self.ime.marked.take().is_some() {
            cx.notify();
        }
        let Some((sid, id, _)) = self.target() else { return };
        self.ime.commits += 1;
        self.write_pane(&sid, &id, text.as_bytes(), cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        selected: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let block = self.target().map(|(_, id, _)| id);
        let end = text.encode_utf16().count();
        let selected = selected.map_or(end..end, |s| s.start.min(end)..s.end.min(end));
        self.ime.marked = block.filter(|_| !text.is_empty()).map(|block| Marked { block, text: text.into(), selected });
        cx.notify();
    }

    /// Where the candidate window goes: under the cursor's cell, a cell further per UTF-16 unit
    /// (shortcut: a wide character puts it a little left of its end, close enough for the list).
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
    /// Bytes for pane `id`, from a key or the input handler: the pane's queue keeps them in order.
    pub(super) fn write_pane(&mut self, sid: &str, id: &str, bytes: &[u8], cx: &mut Context<Self>) {
        // only what reaches the pane is timed (LEON_FRAME_LOG): its echo moves the cursor
        if self.stats.borrow().logging() {
            if let Some(stamped) = crate::input::current_event_stamp() {
                let cursor = self.drawn(id).frame.cursor.map(|c| (c.line, c.col));
                self.stats.borrow_mut().key(stamped, id.to_string(), cursor);
            }
        }
        self.app.write(sid, id, bytes);
        if self.view_top(id).is_some() {
            self.scroll_to(id, None); // typing goes back to the prompt
            cx.notify();
        }
    }

    /// The target pane's cursor cell in the window, where marked text is drawn.
    pub fn ime_anchor(&self) -> Option<Bounds<Pixels>> {
        let (_, id, _) = self.target()?;
        let (frame, _, _) = self.grid_given(&id)?;
        let c = frame.cursor.as_ref()?;
        let p = self.term.painted.borrow().get(&id).copied()?;
        let origin = p.origin + gpui::point(p.cell.0 * c.col as f32, p.cell.1 * c.line as f32);
        Some(Bounds { origin, size: gpui::size(p.cell.0, p.cell.1) })
    }

    /// What the input method is composing on the target pane (tests).
    pub fn marked_text(&self) -> Option<String> {
        self.ime.marked.as_ref().map(|m| m.text.clone())
    }

    /// Texts the input handler wrote to a pane (tests).
    pub fn input_commits(&self) -> u64 {
        self.ime.commits
    }

    /// Marked text ends, unsent, when the focus leaves the pane (`sync_focus`).
    pub(super) fn end_marked(&mut self) {
        self.ime.marked = None;
    }

    /// Registers leon's input handler for the workspace's focus while painting: GPUI keeps it only
    /// while that handle has the focus (the pane), and asks it for the window's next key text.
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
            .text_decoration_1()
            .text_decoration_solid()
            .text_decoration_color(rgb(t.fg))
            .child(m.text.clone());
        Some(text.into_any_element())
    }
}
