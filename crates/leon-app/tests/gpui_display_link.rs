//! A GPUI window keeps one display link (docs/architecture/gpui-patch.md): shown at once 500 times
//! (`displayLayer:`, what a key's echo goes through, view_echo.rs) and moved between screens while
//! on screen, then drawn at once and hidden and shown again while off screen, it still has the one
//! link it started with. GPUI 0.2.2 as published dropped and rebuilt the link each time, and never
//! frees a dropped one. Run without `[patch.crates-io]` this is the check that the patch can go.
//!
//! A real window on the real platform, so its own `main` (AppKit wants the main thread). It opens
//! a window on screen, so it runs only with `LEON_GUI_TESTS=1` (CI's macOS job, the VM) and says
//! it skipped otherwise; there, a window that never gets on screen fails.

const NAME: &str = "display_layer_reuses_the_display_link";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // what cargo-nextest asks a custom harness: the list, then the one test by name
    if args.iter().any(|a| a == "--list") {
        if cfg!(target_os = "macos") && !args.iter().any(|a| a == "--ignored") {
            println!("{NAME}: test");
        }
        return;
    }
    // libtest's own arguments: a name filter, `--skip NAME`, `--ignored` (this test isn't), and
    // the options that take a value
    let (mut filters, mut skips, mut it) = (Vec::new(), Vec::new(), args.iter());
    while let Some(a) = it.next() {
        match a.as_str() {
            "--ignored" => return,
            "--skip" => skips.extend(it.next().map(String::as_str)),
            "--test-threads" | "--format" | "--color" | "--logfile" | "-Z" => drop(it.next()),
            s if s.starts_with('-') => {}
            s => filters.push(s),
        }
    }
    let exact = args.iter().any(|a| a == "--exact");
    let hit = |f: &&str| if exact { NAME == *f } else { NAME.contains(*f) };
    if (!filters.is_empty() && !filters.iter().any(hit)) || skips.iter().any(hit) {
        return;
    }
    run();
}

#[cfg(not(target_os = "macos"))]
fn run() {}

// macOS only: GPUI's macOS platform and its display link are what this measures.
#[cfg(target_os = "macos")]
fn run() {
    use gpui::{px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions};
    use leon_app::view::echo::show_now;
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::time::Duration;

    struct Blank;
    impl gpui::Render for Blank {
        fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    /// GPUI's window says it is on screen, covered or not: GPUI runs a display link only then,
    /// so a window behind another (or a locked screen) can't pass by having no link at all.
    extern "C-unwind" fn on_screen(_: *mut objc2::runtime::AnyObject, _: Sel) -> usize {
        1 << 1 // NSWindowOcclusionStateVisible
    }
    extern "C-unwind" fn hidden(_: *mut objc2::runtime::AnyObject, _: Sel) -> usize {
        0
    }
    /// GPUI's window class answers `occlusionState` with `answer` from now on.
    fn occlusion(answer: extern "C-unwind" fn(*mut objc2::runtime::AnyObject, Sel) -> usize) {
        let class = AnyClass::get(c"GPUIWindow").expect("GPUI's window class") as *const AnyClass as *mut AnyClass;
        // SAFETY: the same signature as -[NSWindow occlusionState]; on the main thread
        unsafe {
            let imp: Imp = std::mem::transmute(answer);
            objc2::ffi::class_replaceMethod(class, objc2::sel!(occlusionState), imp, c"Q@:".as_ptr());
        }
    }

    /// GPUI's NSView of `window`.
    fn ns_view(window: &gpui::Window) -> *mut AnyObject {
        match HasWindowHandle::window_handle(window).map(|h| h.as_raw()) {
            Ok(RawWindowHandle::AppKit(h)) => h.ns_view.as_ptr().cast(),
            _ => panic!("GPUI's window has an NSView"),
        }
    }

    if std::env::var("LEON_GUI_TESTS").is_ok_and(|v| v == "1") {
        println!("{NAME}: runs (LEON_GUI_TESTS=1)");
    } else {
        println!("{NAME}: skipped (opens a window: set LEON_GUI_TESTS=1)");
        return;
    }
    Application::new().run(move |cx: &mut App| {
        occlusion(on_screen); // before the window opens, so GPUI starts its link
        let bounds = Bounds::centered(None, size(px(200.), px(120.)), cx);
        let options =
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), focus: false, ..Default::default() };
        let handle = cx.open_window(options, |_, cx| cx.new(|_| Blank)).expect("a window");
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let tick = || executor.timer(Duration::from_millis(2));
            tick().await;
            let view = handle.update(cx, |_, window, _| ns_view(window)).expect("the window");
            // SAFETY: GPUI's NSView and its window live until the process exits; main thread. The
            // window is its own delegate: these are AppKit's notifications to it, sent by hand.
            let tell = |sel: Sel| unsafe {
                let window: *mut AnyObject = msg_send![view, window];
                let _: *mut AnyObject =
                    msg_send![window, performSelector: sel, withObject: std::ptr::null_mut::<AnyObject>()];
            };
            let mut failed = Vec::new();
            let check = |what: &str, before: usize| {
                let now = display_links();
                println!("{NAME}: {what}: live CVDisplayLinks {before} before, {now} after");
                (now != before).then(|| format!("{what}: {before} links before, {now} after"))
            };
            let before = display_links();
            if before == 0 {
                eprintln!("{NAME}: FAILED: the window on screen has no display link");
                std::process::exit(1);
            }

            // on screen: shown at once through the app's own path, and moved between screens
            let mut asked = 0;
            for _ in 0..500 {
                asked += handle.update(cx, |_, window, _| show_now(window)).expect("the window") as usize;
                tick().await;
            }
            for _ in 0..20 {
                tell(objc2::sel!(windowDidChangeScreen:));
            }
            if asked != 500 {
                failed.push(format!("on screen, {asked} of 500 shows were asked for"));
            }
            failed.extend(check("on screen: 500 shows at once, 20 screen changes", before));

            // off screen: not shown at once, a frame drawn at once anyway (activation does), and
            // hidden and shown 50 times; then on screen again
            occlusion(hidden);
            tell(objc2::sel!(windowDidChangeOcclusionState:));
            if handle.update(cx, |_, window, _| show_now(window)).expect("the window") {
                failed.push("a hidden window was shown at once".into());
            }
            for _ in 0..50 {
                // SAFETY: as above; AppKit calls this when the view's layer needs display
                unsafe {
                    let layer: *mut AnyObject = msg_send![view, layer];
                    let _: () = msg_send![view, displayLayer: layer];
                }
            }
            for _ in 0..50 {
                occlusion(on_screen);
                tell(objc2::sel!(windowDidChangeOcclusionState:));
                occlusion(hidden);
                tell(objc2::sel!(windowDidChangeOcclusionState:));
            }
            occlusion(on_screen);
            tell(objc2::sel!(windowDidChangeOcclusionState:));
            failed.extend(check("off screen: 50 frames at once, hidden and shown 50 times", before));
            for f in &failed {
                eprintln!("{NAME}: FAILED: {f} (GPUI drops and rebuilds the link, and never frees it)");
            }
            let ok = failed.is_empty();
            std::process::exit(if ok { 0 } else { 1 });
        })
        .detach();
    });
}

/// The live CVDisplayLink objects in this process, counted by `heap`.
// macOS only: heap(1) and CVDisplayLink are macOS's.
#[cfg(target_os = "macos")]
fn display_links() -> usize {
    let out = std::process::Command::new("heap").arg(std::process::id().to_string()).output().expect("heap(1) runs");
    assert!(out.status.success(), "heap(1) read this process: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter(|l| l.contains("CVCGDisplayLink") || l.contains("CVDisplayLink"))
        .filter_map(|l| l.split_whitespace().next()?.parse::<usize>().ok())
        .sum()
}
