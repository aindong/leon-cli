//! Starting the app: its key bindings, menus and quit rules, and its window.

use super::*;

/// GPUI key bindings: only Quit. Every other shortcut goes through `key_down` (or the open overlay's
/// own handler, view_overlays.rs) and the keymap
/// (`keymap::DEFAULTS` under the config's bindings), since GPUI matches its own bindings before
/// any key listener and a config could never override them.
pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
}

/// Keys, menus and quitting. Quitting (⌘Q, the menu, the Dock, logout, closing the window) only
/// closes the app's connections: the server and every program keep running.
pub fn install(cx: &mut App) {
    init(cx);
    cx.set_menus(menus());
    cx.on_action(|_: &Quit, cx| cx.quit());
    // closing the Settings window leaves the app running; closing the workspace's quits it
    cx.on_window_closed(|cx| {
        if quits(cx) {
            cx.quit()
        }
    })
    .detach();
}

/// Open the app window on `server` and run until it quits.
pub fn run(server: Remote) {
    let server = Rc::new(server);
    Application::new().with_assets(crate::icons::Icons).run(move |cx: &mut App| {
        install(cx);
        // the last frame, on its display, when it still fits there; else most of the primary one.
        // A window's bounds are relative to its display, so the frame is too.
        let saved = settings_path().map(|p| Settings::load(&p)).unwrap_or_default();
        let on = saved.display.and_then(|id| cx.displays().into_iter().find(|d| u32::from(d.id()) == id));
        let display = on.clone().or_else(|| cx.primary_display());
        let screen = display.as_ref().map(|d| chrome::Frame { x: 0., y: 0., ..state::frame(d.bounds()) });
        let f = chrome::window_frame(on.and(saved.window), screen.as_slice());
        let bounds = Bounds::new(point(px(f.x), px(f.y)), size(px(f.w), px(f.h)));
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("leon".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(13.))),
            }),
            window_min_size: Some(size(px(chrome::MIN_WINDOW.0), px(chrome::MIN_WINDOW.1))),
            display_id: display.map(|d| d.id()),
            ..WindowOptions::default()
        };
        let app = server.clone();
        cx.open_window(options, move |window, cx| cx.new(|cx| Workspace::for_app(app, window, cx)))
            .expect("open the leon window");
        cx.activate(true);
    });
}

/// After a window closed: quit when no workspace window is left (closing Settings doesn't quit).
pub fn quits(cx: &App) -> bool {
    !cx.windows().iter().any(|w| w.downcast::<Workspace>().is_some())
}

impl Workspace {
    /// A workspace whose settings live in memory only.
    pub fn new(app: Rc<Remote>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_settings(app, None, window, cx)
    }

    /// The app's own workspace: settings from `settings_path()`, saved there on each change, and
    /// the hosts list's machines.
    pub fn for_app(app: Rc<Remote>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hosts = leon_hosts::hosts::load().unwrap_or_else(|e| {
            eprintln!("leon: can't read the hosts list: {e}");
            Vec::new()
        });
        let mut ws = Self::with_hosts(app, settings_path(), hosts, window, cx);
        ws.platform = true;
        ws.native_key = crate::input::current_key_event;
        ws.echo.show = echo::show_now;
        let store = leon_core::paths::state_dir().map(|d| d.join("updates.json"));
        ws.enable_updates(store, Arc::new(leon_update::Target::running));
        // the app runs as `leon app`: its own binary says whether an agent's skill is out of date
        if let Some(leon) = std::env::current_exe().ok().filter(|e| e.file_name().is_some_and(|n| n == "leon")) {
            ws.check_agent_skill(leon, cx);
        }
        crate::notify::prepare();
        ws
    }

    /// A workspace with the settings in `path` (the defaults when it is None or unreadable).
    pub fn with_settings(app: Rc<Remote>, path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_hosts(app, path, Vec::new(), window, cx)
    }
}
