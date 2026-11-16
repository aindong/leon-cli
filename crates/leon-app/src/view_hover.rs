//! The hover previews: resting on a tab in the titlebar, or on a tab's row in the sidebar, shows
//! its thumbnail under the tab or beside the row.

use super::*;
use crate::overview;

/// How long the pointer rests on a tab before its preview shows.
pub const HOVER_DELAY: Duration = Duration::from_millis(400);
/// After the preview hides, how long a tab entered still shows its preview at once (the pointer
/// moving from one tab to the next leaves one before it enters the other).
const HOVER_WARM: Duration = Duration::from_millis(150);
/// A hover preview's width.
const PREVIEW_W: f32 = 320.;

/// A tab under the pointer: which, where its preview goes, and whether it shows yet.
#[derive(Clone)]
pub(super) struct Hover {
    pub(super) host: usize,
    pub(super) session_id: String,
    pub(super) window_id: String,
    at: gpui::Point<Pixels>,
    pub(super) shown: bool,
    sidebar: bool,
}

impl Workspace {
    /// The tab whose hover preview shows.
    pub fn hover_preview(&self) -> Option<String> {
        self.overview.hover.as_ref().filter(|h| h.shown).map(|h| h.window_id.clone())
    }

    /// The pointer entered (or left) a tab: after `HOVER_DELAY` its preview shows, at once when
    /// another tab's preview just showed.
    pub(in crate::view) fn hover_tab(
        &mut self,
        on: bool,
        (host, sid, wid): (usize, String, String),
        sidebar: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let same = self.overview.hover.as_ref().is_some_and(|h| h.window_id == wid);
        if !on {
            if same {
                if self.overview.hover.take().is_some_and(|h| h.shown) {
                    self.overview.hover_left = Some(Instant::now());
                }
                cx.notify();
            }
            return;
        }
        let warm = self.overview.hover.as_ref().is_some_and(|h| h.shown)
            || self.overview.hover_left.is_some_and(|t| t.elapsed() < HOVER_WARM);
        if self.overview_shown() || self.palette.is_some() {
            return;
        }
        let at = window.mouse_position();
        self.overview.hover = Some(Hover { host, session_id: sid, window_id: wid, at, shown: warm, sidebar });
        self.overview.hover_gen += 1;
        if !warm {
            let gen = self.overview.hover_gen;
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(HOVER_DELAY).await;
                let _ = this.update(cx, |ws, cx| {
                    if ws.overview.hover_gen == gen {
                        if let Some(h) = ws.overview.hover.as_mut() {
                            h.shown = true;
                            cx.notify();
                        }
                    }
                });
            })
            .detach();
        }
        cx.notify();
    }

    /// The hover preview, under its tab in the titlebar or beside its row in the sidebar.
    pub(in crate::view) fn render_preview(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let h = self.overview.hover.clone().filter(|h| h.shown)?;
        // a tab closed under the pointer sends no leave of its own (its element is gone, or another
        // tab took its id), so its preview goes with it
        let remote = self.hosts.machines().get(h.host).and_then(|m| m.remote()).cloned();
        let alive = remote.is_some_and(|r| {
            let m = r.model();
            m.session(&h.session_id).is_some_and(|s| s.windows.iter().any(|w| w.id == h.window_id))
        });
        // nor does a row or tab whose list went (the sidebar hidden, the tab style switched)
        let places = self.places();
        // the titlebar draws only the selected session's tabs: another session's preview is gone
        let drawn = match h.sidebar {
            true => places.sidebar == Some(SidebarList::SessionsAndTabs),
            false => places.top_tabs && self.session_id.as_deref() == Some(h.session_id.as_str()),
        };
        if !alive || !drawn {
            self.overview.hover = None;
            return None;
        }
        self.overview.area = self.pane_area(window);
        self.refresh_thumbs(false, cx);
        let t = &self.theme;
        let (aw, ah) = self.overview.area;
        let height = overview::HEADER + PREVIEW_W * if aw > 0. { ah / aw } else { 0.6 };
        let view = window.viewport_size();
        let (vw, vh) = (f32::from(view.width), f32::from(view.height));
        let (x, y) = match h.sidebar {
            true => (self.settings.sidebar_width + 8., f32::from(h.at.y) - overview::HEADER),
            false => (f32::from(h.at.x) - PREVIEW_W / 2., TITLEBAR + 4.),
        };
        let (x, y) = (x.clamp(8., (vw - PREVIEW_W - 8.).max(8.)), y.clamp(8., (vh - height - 8.).max(8.)));
        let rows = self.rows();
        let title = rows.iter().flat_map(|r| &r.windows).find(|w| w.id == h.window_id).map(|w| w.title.clone());
        let name = format!("hover-pane-{}", h.window_id);
        Some(
            div()
                .debug_selector(|| sel("hover-preview".into()))
                .absolute()
                .left(px(x))
                .top(px(y))
                .w(px(PREVIEW_W))
                .h(px(height))
                .flex()
                .flex_col()
                .rounded(px(6.))
                .overflow_hidden()
                .bg(rgb(t.bg))
                .border_1()
                .border_color(rgb(t.border))
                .shadow_lg()
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(px(overview::HEADER))
                        .px(px(10.))
                        .text_color(rgb(t.title))
                        .child(title.unwrap_or_default()),
                )
                .child(
                    div().debug_selector(move || sel(name)).flex_1().relative().children(self.thumb_view(&h.window_id)),
                )
                .into_any_element(),
        )
    }
}
