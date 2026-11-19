//! GPUI's `displayLayer:` (what a key's echo shown at once goes through, view_echo.rs) reuses the
//! window's display link: 500 calls leave the one link, where GPUI 0.2.2 as published leaked one
//! per call (docs/architecture/gpui-patch.md). Run without `[patch.crates-io]` this is the check
//! that the patch can go.
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
    let filters: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if !filters.is_empty() && !filters.iter().any(|f| NAME.contains(f.as_str())) {
        return;
    }
    run();
}

// macOS only: GPUI's macOS platform and its display link are what this measures.
#[cfg(not(target_os = "macos"))]
fn run() {}

#[cfg(target_os = "macos")]
fn run() {
    use gpui::{px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions};
    use leon_app::view::echo::show_now;
    use objc2::runtime::{AnyClass, Imp, Sel};
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
            let before = display_links();
            let mut asked = 0;
            for _ in 0..500 {
                // the app's own path: AppKit displays the view at the end of this run-loop pass
                asked += handle.update(cx, |_, window, _| show_now(window)).expect("the window") as usize;
                tick().await;
            }
            let after = display_links();
            println!("{NAME}: live CVDisplayLinks {before} before, {after} after {asked} immediate shows");
            let mut ok = asked == 500 && before >= 1 && after <= before + 1;
            if !ok {
                eprintln!(
                    "{NAME}: FAILED: each displayLayer: left a display link behind (GPUI rebuilds it, not reuses it)"
                );
            }
            // a window not on screen is not shown at once: GPUI draws nothing there
            occlusion(hidden);
            if handle.update(cx, |_, window, _| show_now(window)).expect("the window") {
                eprintln!("{NAME}: FAILED: a hidden window was shown at once");
                ok = false;
            }
            std::process::exit(if ok { 0 } else { 1 });
        })
        .detach();
    });
}

/// The live CVDisplayLink objects in this process, counted by `heap`.
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
