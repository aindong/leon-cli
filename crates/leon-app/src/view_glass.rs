//! The translucent window in the workspace (glass.rs has the alphas): what GPUI is asked for the
//! window's background (`Window::set_background_appearance`, window.rs), and the fills the views
//! draw with. Every pixel gets one translucent fill at most: the root fills nothing then, and the
//! sidebar, the title bar, the panes area's gaps and each tile fill themselves.

use gpui::{quad, AnyWindowHandle, BorderStyle, ContentMask, Hsla, WindowBackgroundAppearance as Bg};

use super::*;
use crate::glass::{gaps, Gaps, Glass};

/// A tile's corner radius (`pane_tile`'s `rounded`): the gaps go round it.
const TILE_RADIUS: f32 = 6.;
/// How many backgrounds asked for the log keeps (tests read it; it changes only with a setting).
const KEEP: usize = 16;

pub(super) struct GlassUi {
    glass: Glass,
    /// What GPUI was asked for, oldest first: GPUI's test window keeps no background.
    set: Vec<Bg>,
    /// Reduce Transparency as set by a test; None asks the system.
    reduce_transparency: Option<bool>,
    /// Reduce Transparency when last asked (on activation: it is changed in System Settings).
    reduced: bool,
    /// The workspace's window, for a look from where there is no window at hand.
    window: AnyWindowHandle,
}

impl GlassUi {
    pub(super) fn new(window: AnyWindowHandle) -> Self {
        GlassUi { glass: Glass::OPAQUE, set: Vec::new(), reduce_transparency: None, reduced: false, window }
    }
}

/// `color` with `alpha`.
fn see_through(color: u32, alpha: f32) -> Hsla {
    Hsla::from(rgb(color)).opacity(alpha)
}

impl Workspace {
    /// How opaque the window's fills are drawn now.
    pub fn glass(&self) -> Glass {
        self.glass.glass
    }

    /// The window backgrounds GPUI was asked for, oldest first.
    pub fn backgrounds_set(&self) -> &[Bg] {
        &self.glass.set
    }

    /// Use Reduce Transparency as `on` says whatever the system does (tests); None asks the system.
    pub fn set_reduce_transparency(&mut self, on: Option<bool>) {
        self.glass.reduce_transparency = on;
    }

    /// Ask whether Reduce Transparency is on (the system, unless a test set it).
    pub(super) fn note_reduce_transparency(&mut self) {
        self.glass.reduced = self.glass.reduce_transparency.unwrap_or_else(floats_view::system_reduce_transparency);
    }

    /// Look again at what the window shows through: the setting, unless Reduce Transparency is on
    /// or the window is in full screen (nothing is behind it to see). Called where those change:
    /// the window's activation (back from System Settings), bounds (full screen), appearance (the
    /// theme), and after a settings reload (`glass_later`). GPUI is asked only for a change.
    pub fn update_glass(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let see = if self.glass.reduced || window.is_fullscreen() { 0. } else { self.look.translucency };
        let glass = Glass::new(&self.theme, see);
        let bg = if glass.translucent() { Bg::Blurred } else { Bg::Opaque };
        if self.glass.set.last() != Some(&bg) {
            window.set_background_appearance(bg);
            if self.glass.set.len() == KEEP {
                self.glass.set.remove(0);
            }
            self.glass.set.push(bg);
        }
        if glass != self.glass.glass {
            self.glass.glass = glass;
            cx.notify();
        }
    }

    /// [`Workspace::update_glass`] once this update is over, through the window's handle: a
    /// settings reload comes from the poll, which has no window.
    pub(super) fn glass_later(&self, cx: &mut Context<Self>) {
        let (handle, this) = (self.glass.window, cx.entity().downgrade());
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| this.update(cx, |ws, cx| ws.update_glass(window, cx)));
        });
    }

    /// The window's own fill: the chrome when opaque; nothing when translucent.
    pub(super) fn root_fill(&self) -> Hsla {
        if self.glass.glass.translucent() {
            Hsla::default()
        } else {
            rgb(self.theme.chrome).into()
        }
    }

    /// The sidebar's, the title bar's and the gaps' fill.
    pub(super) fn chrome_fill(&self) -> Hsla {
        see_through(self.theme.chrome, self.glass.glass.chrome_alpha())
    }

    /// A tile's fill in its own theme `t`. A float is opaque: it sits over other tiles.
    pub(super) fn pane_fill(&self, kind: &str, t: &Theme) -> Hsla {
        let alpha = if kind == "float" { 1. } else { self.glass.glass.pane_alpha(t) };
        see_through(t.bg, alpha)
    }

    /// The panes area's chrome: all of it when opaque; when translucent, only what the tiles and
    /// drawers in `l` and `placed` leave (they fill themselves), corners included.
    pub(super) fn render_gaps(&self, l: &panes::Layout, placed: &floats::Placed, terms: &Panes) -> impl IntoElement {
        let mut tiles: Vec<panes::Rect> =
            l.panes.iter().filter(|p| terms.contains(&p.block_id)).map(|p| p.rect).collect();
        tiles.extend(placed.docks.iter().filter(|d| terms.contains(&d.block_id)).map(|d| d.rect));
        let (color, translucent) = (self.chrome_fill(), self.glass.glass.translucent());
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let o = bounds.origin;
                let at = |r: &panes::Rect| Bounds::new(point(o.x + px(r.x), o.y + px(r.y)), size(px(r.w), px(r.h)));
                let area = panes::Rect { x: 0., y: 0., w: bounds.size.width.into(), h: bounds.size.height.into() };
                let g = if translucent {
                    gaps(area, &tiles, TILE_RADIUS)
                } else {
                    Gaps { rects: vec![area], ..Gaps::default() }
                };
                for r in &g.rects {
                    window.paint_quad(fill(at(r), color));
                }
                // outside a rounded corner: a ring from the corner's circle outward, cut to the corner
                for c in &g.corners {
                    let r = c.radius;
                    let ring = panes::Rect { x: c.center.0 - 2. * r, y: c.center.1 - 2. * r, w: 4. * r, h: 4. * r };
                    let mask = Some(ContentMask { bounds: at(&c.clip) });
                    window.with_content_mask(mask, |window| {
                        let q = quad(at(&ring), px(2. * r), Hsla::default(), px(r), color, BorderStyle::Solid);
                        window.paint_quad(q)
                    });
                }
            },
        )
        .absolute()
        .size_full()
    }
}
