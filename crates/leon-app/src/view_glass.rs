//! The translucent window in the workspace (glass.rs has the alphas): what GPUI is asked for the
//! window's background (`Window::set_background_appearance`, window.rs), and the fills the views
//! draw with. Every pixel gets one translucent fill at most: the root fills nothing then, and the
//! sidebar, the title bar, the panes area's gaps and each tile fill themselves.

use gpui::{quad, AnyWindowHandle, BorderStyle, ContentMask, Hsla, WindowBackgroundAppearance as Bg};

use super::*;
use crate::glass::{gaps, see_through_tiles, Gaps, Glass, TILE_RADIUS};

/// How many backgrounds asked for the log keeps (tests read it; it changes only with a setting).
const KEEP: usize = 16;

pub(super) struct GlassUi {
    glass: Glass,
    /// What GPUI was asked for, oldest first: GPUI's test window keeps no background.
    set: Vec<Bg>,
    /// How many blur views had their material set after each `Blurred`, oldest first.
    blur_views: Vec<usize>,
    /// Reduce Transparency as set by a test; None asks the system.
    reduce_transparency: Option<bool>,
    /// Reduce Transparency when last asked (on activation: it is changed in System Settings).
    reduced: bool,
    /// The server's settings have been applied: until then the setting isn't known, and a
    /// window set to 0 must never show the desktop, so it is opaque.
    known: bool,
    /// The workspace's window, for a look from where there is no window at hand.
    window: AnyWindowHandle,
}

impl GlassUi {
    pub(super) fn new(window: AnyWindowHandle) -> Self {
        GlassUi {
            glass: Glass::OPAQUE,
            set: Vec::new(),
            blur_views: Vec::new(),
            reduce_transparency: None,
            reduced: false,
            known: false,
            window,
        }
    }
}

/// Keep at most `KEEP` of `log`, then add `x`.
fn keep<T>(log: &mut Vec<T>, x: T) {
    if log.len() == KEEP {
        log.remove(0);
    }
    log.push(x);
}

/// GPUI 0.2.2's blur view (class `BlurredView`) has the `Selection` material, which macOS 27
/// doesn't blur: give each in the app `UnderWindowBackground` (21), as upstream GPUI's
/// `90d8514059` does (docs/architecture/gpui.md, Allowed outside GPUI). Main thread only; GPUI
/// makes the view in `set_background_appearance(Blurred)`. Returns how many it set.
#[cfg(target_os = "macos")]
fn set_blur_material() -> usize {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    const UNDER_WINDOW_BACKGROUND: isize = 21;
    let (Some(blur), Some(app)) = (AnyClass::get(c"BlurredView"), AnyClass::get(c"NSApplication")) else {
        return 0;
    };
    let mut n = 0;
    unsafe {
        let app: *mut AnyObject = msg_send![app, sharedApplication];
        let windows: *mut AnyObject = msg_send![app, windows];
        let count: usize = msg_send![windows, count];
        for i in 0..count {
            let w: *mut AnyObject = msg_send![windows, objectAtIndex: i];
            let content: *mut AnyObject = msg_send![w, contentView];
            if content.is_null() {
                continue;
            }
            let views: *mut AnyObject = msg_send![content, subviews];
            let count: usize = msg_send![views, count];
            for j in 0..count {
                let v: *mut AnyObject = msg_send![views, objectAtIndex: j];
                let is_blur: Bool = msg_send![v, isKindOfClass: blur];
                if !is_blur.as_bool() {
                    continue; // GPUI's own view: no `material`
                }
                let material: isize = msg_send![v, material];
                if material != UNDER_WINDOW_BACKGROUND {
                    let _: () = msg_send![v, setMaterial: UNDER_WINDOW_BACKGROUND];
                    n += 1;
                }
            }
        }
    }
    n
}

#[cfg(not(target_os = "macos"))]
fn set_blur_material() -> usize {
    0
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

    /// How many blur views had their material set after each `Blurred`, oldest first.
    pub fn blur_views_set(&self) -> &[usize] {
        &self.glass.blur_views
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
        let see =
            if !self.glass.known || self.glass.reduced || window.is_fullscreen() { 0. } else { self.look.translucency };
        let glass = Glass::new(&self.theme, see, self.look.fade);
        let bg = if glass.translucent() { Bg::Blurred } else { Bg::Opaque };
        if self.glass.set.last() != Some(&bg) {
            window.set_background_appearance(bg);
            keep(&mut self.glass.set, bg);
            if bg == Bg::Blurred {
                // each Blurred after an Opaque is a new view, with GPUI's material again
                let n = set_blur_material();
                if n == 0 && cfg!(target_os = "macos") {
                    static ONCE: std::sync::Once = std::sync::Once::new();
                    ONCE.call_once(|| {
                        eprintln!("leon: no blur view from GPUI to set; the desktop shows through unblurred")
                    });
                }
                keep(&mut self.glass.blur_views, n);
            }
        }
        if glass != self.glass.glass {
            self.glass.glass = glass;
            cx.notify();
        }
    }

    /// [`Workspace::update_glass`] once this update is over, through the window's handle: a
    /// settings reload, applied just before, comes from the poll, which has no window.
    pub(super) fn glass_later(&mut self, cx: &mut Context<Self>) {
        self.glass.known = true;
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

    /// A tile's fill in its own theme `t` (faded or not; every pane has one alpha). A float is
    /// opaque: it sits over other tiles.
    pub(super) fn pane_fill(&self, kind: &str, t: &Theme) -> Hsla {
        let alpha = if kind == "float" { 1. } else { self.glass.glass.pane_alpha() };
        see_through(t.bg, alpha)
    }

    /// The panes area's chrome: all of it when opaque; when translucent, only what the tiles and
    /// drawers in `l` and `placed` leave (they fill themselves), corners included.
    pub(super) fn render_gaps(&self, l: &panes::Layout, placed: &floats::Placed, terms: &Panes) -> impl IntoElement {
        let tiles = see_through_tiles(l, placed, |id| terms.iter().any(|t| t == id));
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
