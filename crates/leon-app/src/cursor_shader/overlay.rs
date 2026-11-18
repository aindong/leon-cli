//! The overlay on a window (docs/architecture/cursor-shaders.md): a transparent `CAMetalLayer`
//! added as a sublayer of GPUI's view's layer, drawn by a `CVDisplayLink` of its own that runs
//! only while a cursor change animates, on the link's thread, never the main thread. A sublayer
//! is not a view: AppKit sends events to views only, so it never takes one. It never asks GPUI
//! for a frame (ADR R2). Each workspace owns its window's; on GPUI's test platform, which has no
//! AppKit view, it has no layer and keeps the cursor state only, which tests read.

use std::ffi::c_void;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use core_video::display_link::{
    CVDisplayLinkCreateWithActiveCGDisplays, CVDisplayLinkIsRunning, CVDisplayLinkRef, CVDisplayLinkRelease,
    CVDisplayLinkSetOutputCallback, CVDisplayLinkStart, CVDisplayLinkStop,
};
use metal::foreign_types::ForeignTypeRef;
use metal::{CommandQueue, MTLPixelFormat, MetalLayer};
use objc2::msg_send;
use objc2::runtime::{AnyClass, AnyObject};

use super::program::{clear, Program};
use super::{Cursor, Drawn, Next};
use crate::theme::Theme;

struct Gpu {
    layer: MetalLayer,
    queue: CommandQueue,
    link: Link,
    program: Arc<Program>,
}

/// The display link, and the overlay count its callback holds (released in `detach`).
#[derive(Clone, Copy)]
struct Link {
    link: CVDisplayLinkRef,
    user: *const Overlay,
}

// SAFETY: CoreVideo's display link functions may be called from any thread, its own callback's
// included; `user` is only turned back into its Arc once the link is stopped (`detach`)
unsafe impl Send for Link {}
unsafe impl Sync for Link {}

/// kCALayerWidthSizable | kCALayerHeightSizable: the layer follows GPUI's through resizes.
const SIZABLE: u32 = (1 << 1) | (1 << 4);

/// One window's overlay.
pub struct Overlay {
    cursor: Mutex<Cursor>,
    gpu: Mutex<Option<Gpu>>,
    /// `LEON_FRAME_STATS=1`: a line per animation (frames, encode time), for scripts/idle-frames.sh.
    stats: Option<Mutex<(u32, Duration, Duration)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Equal only to itself: a grid's input holds its window's overlay, and compares by which.
impl PartialEq for Overlay {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl Overlay {
    /// An overlay with no layer yet: it keeps the cursor state until `attach` gives it a program.
    pub fn new() -> Arc<Self> {
        Arc::new(Overlay {
            cursor: Mutex::new(Cursor::new(Instant::now())),
            gpu: Mutex::new(None),
            stats: (std::env::var("LEON_FRAME_STATS").as_deref() == Ok("1")).then(Mutex::default),
        })
    }

    /// Draw `program` over `window`: the layer and link made on the first, the program replaced
    /// after. Main thread.
    pub fn attach(self: &Arc<Self>, window: &gpui::Window, program: Arc<Program>) -> Result<(), String> {
        let mut gpu = lock(&self.gpu);
        if let Some(g) = gpu.as_mut() {
            g.program = program;
            return Ok(());
        }
        *gpu = Some(make(window, self, program)?);
        drop(gpu);
        self.fit(window);
        Ok(())
    }

    /// Take the layer away: its link stopped and released, its layer off GPUI's. Main thread.
    pub fn detach(&self) {
        // out of the lock first: Stop waits for a tick in progress, which may want that lock
        let gpu = lock(&self.gpu).take();
        let Some(g) = gpu else { return };
        // SAFETY: a link this module made, stopped before it is released, so no tick runs when
        // the callback's overlay count is given back; the layer is ours, on the main thread
        unsafe {
            CVDisplayLinkStop(g.link.link);
            CVDisplayLinkRelease(g.link.link);
            drop(Arc::from_raw(g.link.user));
            let _: () = msg_send![g.layer.as_ref().as_ptr() as *mut AnyObject, removeFromSuperlayer];
        }
    }

    /// How long a change animates, and the theme's colours.
    pub fn configure(&self, duration: Duration, theme: &Theme) {
        let mut c = lock(&self.cursor);
        c.duration = duration;
        c.set_theme(theme);
    }

    /// Draw or not: the window is active, Reduce Motion is off, nothing covers the panes. Off
    /// ends a running animation with a clear frame, after which the link stops.
    pub fn enable(&self, on: bool) {
        lock(&self.cursor).enable(on, Instant::now());
    }

    /// The cursor the focused pane's grid just painted (None: none, it is hidden). A move starts
    /// the animation and the link. Main thread (it may fit the layer).
    pub fn feed(&self, window: &gpui::Window, drawn: Option<&Drawn>) {
        let mut c = lock(&self.cursor);
        if !c.feed(drawn, Instant::now()) {
            return;
        }
        // started under the cursor's lock: a tick that decided to stop did so under it too, so
        // this start comes after that stop, never before it
        let link = lock(&self.gpu).as_ref().map(|g| g.link);
        if let Some(l) = link {
            // SAFETY: a link this module made, alive while `gpu` holds it
            unsafe {
                if CVDisplayLinkIsRunning(l.link) == 0 {
                    CVDisplayLinkStart(l.link);
                }
            }
        }
        drop(c);
        self.fit(window);
    }

    /// The cursor state (tests).
    pub fn state(&self) -> Cursor {
        lock(&self.cursor).clone()
    }

    /// Match the layer to GPUI's view: its frame, Retina scale and pixel size. Main thread:
    /// the bounds observer (resize, full screen, another display) and each animation's start.
    pub fn fit(&self, window: &gpui::Window) {
        let gpu = lock(&self.gpu);
        let Some(g) = gpu.as_ref() else { return };
        let scale = window.scale_factor() as f64;
        let size = window.viewport_size();
        let (w, h) = (f32::from(size.width) as f64, f32::from(size.height) as f64);
        let ml = g.layer.as_ref().as_ptr() as *mut AnyObject;
        // SAFETY: our own layer, on the main thread; no implicit animation of the change
        unsafe {
            let Some(ca) = AnyClass::get(c"CATransaction") else { return };
            let _: () = msg_send![ca, begin];
            let _: () = msg_send![ca, setDisableActions: true];
            let _: () = msg_send![ml, setFrame: CgRect { x: 0., y: 0., w, h }];
            let _: () = msg_send![ca, commit];
        }
        g.layer.set_contents_scale(scale);
        let now = g.layer.drawable_size();
        if (now.width, now.height) != (w * scale, h * scale) {
            // SAFETY: as above
            unsafe {
                let _: () = msg_send![ml, setDrawableSize: CgSize { w: w * scale, h: h * scale }];
            }
        }
    }
}

fn make(window: &gpui::Window, o: &Arc<Overlay>, program: Arc<Program>) -> Result<Gpu, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?;
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return Err("not an AppKit window".into()) };
    let layer = MetalLayer::new();
    layer.set_device(&program.device);
    layer.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    layer.set_opaque(false);
    layer.set_framebuffer_only(true);
    // SAFETY: `ns_view` is GPUI's live NSView (we are on the main thread, inside its window's
    // callbacks); it is layer-backed
    let host: *mut AnyObject = unsafe { msg_send![h.ns_view.as_ptr() as *mut AnyObject, layer] };
    if host.is_null() {
        return Err("GPUI's view has no layer".into());
    }
    let mut link = std::ptr::null_mut();
    // SAFETY: an out-pointer for a new link; checked for null below
    unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut link) };
    if link.is_null() {
        return Err("no display link".into());
    }
    // the callback's own count of the overlay, given back by `detach`
    let user = Arc::into_raw(o.clone());
    // SAFETY: `tick` reads `user` as the overlay, which that count keeps alive until `detach`
    // stops the link; adding a sublayer changes nothing of GPUI's own layer
    unsafe {
        CVDisplayLinkSetOutputCallback(link, tick, user as *mut c_void);
        let ml = layer.as_ref().as_ptr() as *mut AnyObject;
        let _: () = msg_send![ml, setAutoresizingMask: SIZABLE];
        let _: () = msg_send![host, addSublayer: ml];
    }
    Ok(Gpu { queue: program.device.new_command_queue(), layer, link: Link { link, user }, program })
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CgRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CgSize {
    w: f64,
    h: f64,
}

// SAFETY: the layouts of CGRect and CGSize on 64-bit macOS
unsafe impl objc2::Encode for CgSize {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl objc2::Encode for CgRect {
    const ENCODING: objc2::Encoding = objc2::Encoding::Struct(
        "CGRect",
        &[
            objc2::Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]),
            objc2::Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]),
        ],
    );
}

extern "C" fn tick(
    _: CVDisplayLinkRef,
    _: *const core_video::base::CVTimeStamp,
    _: *const core_video::base::CVTimeStamp,
    _: core_video::base::CVOptionFlags,
    _: *mut core_video::base::CVOptionFlags,
    user: *mut c_void,
) -> core_video::r#return::CVReturn {
    // SAFETY: `user` is the overlay count `make` gave the link, alive until `detach` stops it
    let o = unsafe { &*(user as *const Overlay) };
    // the link's thread has no autorelease pool of its own for the drawables and buffers
    objc2::rc::autoreleasepool(|_| o.frame());
    0
}

impl Overlay {
    /// One display-link tick: draw a frame of the animation, or its last clear frame, or stop.
    fn frame(&self) {
        debug_assert!(objc2::MainThreadMarker::new().is_none(), "the overlay never draws on the main thread");
        // what to draw with, held by this tick: `nextDrawable` can wait, and never under a lock
        // the main thread takes
        let (layer, queue, program, link) = {
            let gpu = lock(&self.gpu);
            let Some(g) = gpu.as_ref() else { return };
            (g.layer.clone(), g.queue.clone(), g.program.clone(), g.link)
        };
        let t0 = Instant::now();
        let (next, mut u) = {
            let mut c = lock(&self.cursor);
            let next = c.next(t0);
            if next == Next::Stop {
                // under the cursor's lock, so a feed's start can't fall between deciding and stopping
                // SAFETY: a link this module made, alive while this tick runs (`detach` waits)
                unsafe { CVDisplayLinkStop(link.link) };
                return;
            }
            (next, c.u)
        };
        let Some(drawable) = layer.next_drawable() else {
            if next == Next::Clear {
                lock(&self.cursor).until = Some(t0); // the clear frame on the next tick
            }
            return;
        };
        let target = drawable.texture();
        u.set_size(target.width() as f32, target.height() as f32);
        let cmd = queue.new_command_buffer();
        match next {
            Next::Draw => program.encode(cmd, target, &u, None),
            _ => clear(cmd, target),
        }
        cmd.present_drawable(drawable);
        cmd.commit();
        if let Some(stats) = &self.stats {
            let mut s = lock(stats);
            let spent = t0.elapsed();
            *s = (s.0 + 1, s.1 + spent, s.2.max(spent));
            if next == Next::Clear {
                let (n, all, max) = std::mem::take(&mut *s);
                let avg = all.as_secs_f64() * 1e6 / n.max(1) as f64;
                eprintln!(
                    "leon cursor-shader: {n} frames, encode avg {avg:.0} µs, max {:.0} µs",
                    max.as_secs_f64() * 1e6
                );
            }
        }
    }
}
