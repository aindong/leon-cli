//! The overlays' focus and keys (ADR R3, R9). The palette (with the switcher, the theme list and
//! Add Host's prompt), the overview and the update notice are views of their own; the find bar
//! and the rename field are parts of the header and sidebar sections. Each overlay that takes keys
//! has its own focus handle and key context and takes its keys from its own `on_key_down`, so
//! `key_down` is the terminal's path only. After every change the workspace gives the focus to
//! the top open overlay, else to itself (the pane), whichever way an overlay closed.

use gpui::{AnyView, Entity, FocusHandle, Keystroke, WeakEntity};

use super::*;

/// An overlay drawn by a view of its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum OverlayKind {
    Palette,
    Overview,
    Notice,
}

/// An overlay view: it draws through the workspace, as the sections do. Not cached: the overlays
/// fade and scale on the root's frames, and GPUI renders the root on every frame it draws.
pub(super) struct OverlayView {
    ws: WeakEntity<Workspace>,
    kind: OverlayKind,
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drawing(cx);
        let kind = self.kind;
        let drawn = self.ws.update(cx, |ws, cx| ws.draw_overlay(kind, window, cx));
        drawn.ok().flatten().unwrap_or_else(|| div().into_any_element())
    }
}

/// The overlays' focus handles and views.
pub(super) struct Overlays {
    pub(super) palette_focus: FocusHandle,
    pub(super) overview_focus: FocusHandle,
    pub(super) find_focus: FocusHandle,
    pub(super) rename_focus: FocusHandle,
    palette: Entity<OverlayView>,
    overview: Entity<OverlayView>,
    notice: Entity<OverlayView>,
}

impl Overlays {
    pub(super) fn new(cx: &mut Context<Workspace>) -> Self {
        let mut view = |kind| {
            let ws = cx.entity().downgrade();
            cx.new(|_| OverlayView { ws, kind })
        };
        let (palette, overview, notice) =
            (view(OverlayKind::Palette), view(OverlayKind::Overview), view(OverlayKind::Notice));
        Overlays {
            palette_focus: cx.focus_handle(),
            overview_focus: cx.focus_handle(),
            find_focus: cx.focus_handle(),
            rename_focus: cx.focus_handle(),
            palette,
            overview,
            notice,
        }
    }

    fn view(&self, kind: OverlayKind) -> &Entity<OverlayView> {
        match kind {
            OverlayKind::Palette => &self.palette,
            OverlayKind::Overview => &self.overview,
            OverlayKind::Notice => &self.notice,
        }
    }
}

impl Workspace {
    /// Overlay `kind`'s view when `shown`, for the root to draw; when not, what it drew last is
    /// forgotten (tests ask what is drawn).
    pub(super) fn overlay(&self, kind: OverlayKind, shown: bool) -> Option<gpui::AnyElement> {
        let view = self.overlays.view(kind);
        if !shown {
            drawn_view::forget(view.entity_id());
            return None;
        }
        Some(AnyView::from(view.clone()).into_any_element())
    }

    fn draw_overlay(
        &mut self,
        kind: OverlayKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        match kind {
            OverlayKind::Palette => self.render_palette_overlay(cx),
            OverlayKind::Overview => Some(self.render_overview(window, cx)),
            OverlayKind::Notice => self.render_update(cx),
        }
    }

    /// Give the focus to the top open overlay that takes keys (the rename field, the palette, the
    /// overview, then the find bar on the pane shown), else to the workspace, which the terminal's
    /// keys go through. Never from a popover (the link card, a row's menu: they keep their own
    /// and give it back when they close) or from another window's element. After every notify.
    pub(super) fn sync_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let o = &self.overlays;
        let finding = self.term.find.as_ref().is_some_and(|f| Some(&f.block) == self.target_block().as_ref());
        let wanted = match () {
            _ if self.stopped => &self.focus, // nothing else is drawn
            _ if self.term.rows.renaming.is_some() => &o.rename_focus,
            _ if self.palette.is_some() => &o.palette_focus,
            _ if self.overview_open() => &o.overview_focus,
            _ if finding => &o.find_focus,
            _ => &self.focus,
        };
        let ours = [&self.focus, &o.palette_focus, &o.overview_focus, &o.find_focus, &o.rename_focus];
        let free = window.focused(cx).is_none_or(|f| ours.iter().any(|h| **h == f));
        if free && !wanted.is_focused(window) {
            window.focus(wanted);
        }
    }

    /// Who has the window's focus (tests): `pane` (the workspace's handle), an overlay's name, or
    /// `popover` (the link card, a row's menu).
    pub fn focus_owner(&self, window: &Window, cx: &App) -> Option<&'static str> {
        let f = window.focused(cx)?;
        let o = &self.overlays;
        Some(match () {
            _ if f == self.focus => "pane",
            _ if f == o.palette_focus => "palette",
            _ if f == o.overview_focus => "overview",
            _ if f == o.find_focus => "find",
            _ if f == o.rename_focus => "rename",
            _ => "popover",
        })
    }

    /// A key at an overlay, as `key_down` reads one: the hover card closes, and the native event's
    /// text is used (`input::retext`).
    fn overlay_keystroke(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) -> Keystroke {
        self.close_hover_card(window, cx);
        match (self.native_key)() {
            Some((code, text)) => crate::input::retext(ev.keystroke.clone(), code, &text),
            None => ev.keystroke.clone(),
        }
    }

    /// The palette's keys: every one is its own.
    pub(super) fn palette_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = self.overlay_keystroke(ev, window, cx);
        self.palette_key(&ks, keymap::chord(&ks).as_ref(), window, cx);
        cx.stop_propagation();
    }

    /// The overview's keys: every one is its own.
    pub(super) fn overview_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = self.overlay_keystroke(ev, window, cx);
        self.overview_key(&ks, keymap::chord(&ks).as_ref(), cx);
        cx.stop_propagation();
    }

    /// The find bar's keys; the rest (a ⌘ chord, a binding) go on to the workspace.
    pub(super) fn find_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = self.overlay_keystroke(ev, window, cx);
        if self.find_key(&ks, cx) {
            cx.stop_propagation();
        }
    }

    /// The rename field's keys: every one is its own.
    pub(super) fn rename_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = self.overlay_keystroke(ev, window, cx);
        self.rename_key(&ks, cx);
        cx.stop_propagation();
    }
}
