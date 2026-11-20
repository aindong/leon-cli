//! Making the workspace: its remotes, hosts, wake task, observers and every field's first value.

use super::*;

impl Workspace {
    /// A workspace on This Mac's `app` that also shows `hosts`, connecting to each.
    pub fn with_hosts(
        app: Rc<Remote>,
        path: Option<PathBuf>,
        hosts: Vec<leon_hosts::Host>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // shortcut: the first session is made before the window draws, waiting for the server
        // (the window has nothing to show without one); every call after it queues. Make it
        // through the queue, drawing the empty state meanwhile, if a slow start is ever seen
        if app.model().sessions.is_empty() {
            let _ = app.act("session.create", json!({}));
        }
        if let Some(queue) = app.call_queue() {
            cx.background_executor().spawn(queue).detach();
        }
        app.commit(); // the window's copy of the model, moved on only by the wake task
        let focus = cx.focus_handle();
        window.focus(&focus);
        // the reader thread wakes the window at once; one pending wake is enough (one sender, so the
        // channel's bound holds: each clone would bring a slot of its own)
        let (wake, woken) = futures::channel::mpsc::channel::<()>(0);
        let wake = Mutex::new(wake);
        let wake: Arc<dyn Fn() + Send + Sync> =
            Arc::new(move || drop(wake.lock().unwrap_or_else(|e| e.into_inner()).try_send(())));
        let woke = wake.clone();
        app.on_change(move || woke());
        crate::notify::on_click(&wake);
        let term = scroll::TermUi { wake: Some(wake.clone()), ..Default::default() };
        let updates =
            Arc::new(crate::updates::Updater::new(None, Arc::new(leon_update::Target::running), wake.clone()));
        let hosts = Hosts::new(app.clone(), hosts, wake.clone());
        let wakes = Self::start_wakes(woken, window, cx);
        let clocks = Self::start_clocks(window, cx);
        let stats = Stats::new();
        let session_id = app.model().sessions.first().map(|s| s.id.clone());
        let activation = cx.observe_window_activation(window, |ws, window, cx| {
            ws.window_active = window.is_window_active();
            ws.note_reduce_transparency();
            ws.note_reduce_motion();
            ws.update_glass(window, cx);
            cx.notify()
        });
        // the window's frame and display are kept with the settings, and saved when the app quits
        // or the window closes (closing drops the workspace before the quit it asks for)
        let bounds = cx.observe_window_bounds(window, |ws, window, cx| {
            ws.settings.window = Some(state::frame(window.window_bounds().get_bounds()));
            ws.settings.display = window.display(cx).map(|d| u32::from(d.id()));
            ws.update_glass(window, cx); // full screen in or out
            ws.fit_cursor_shader(window);
            cx.notify(); // the headers' titles are cut to the panes' widths
        });
        let quit = cx.on_app_quit(|ws, _| {
            ws.save_settings();
            async {}
        });
        let release = cx.on_release(|ws, _| {
            ws.save_settings();
            ws.drop_cursor_shader();
        });
        let appearance = cx.observe_window_appearance(window, |ws, window, cx| {
            ws.set_appearance(look_view::is_dark(window.appearance()), cx);
            ws.update_glass(window, cx)
        });
        // every notify of the workspace (its own state, or what the root draws from a model
        // event, view_feed.rs) brings what render draws from up to date (R5), and gives each grid
        // and section its input; a pushed screen reaches its grid from its model event instead
        // a press that changes nothing (blank chrome) focuses the workspace's handle: the top open
        // overlay gets it back
        let refocus = cx.on_focus(&focus, window, |ws, window, cx| ws.sync_focus(window, cx));
        let lost =
            cx.on_focus_lost(window, |_, window, cx| cx.defer_in(window, |ws, window, cx| ws.focus_lost(window, cx)));
        let prepare = cx.observe_in(&cx.entity(), window, |ws, _, window, cx| {
            ws.wakes.1 += 1;
            ws.prepare();
            ws.root_seen = ws.root_input();
            ws.sync_views(window, cx);
            ws.sync_focus(window, cx);
        });
        let (look, dark) = (Look::default(), look_view::is_dark(window.appearance()));
        let mut ws = Workspace {
            app,
            hosts,
            host: 0,
            select_later: Arc::default(),
            drag: None,
            session_id,
            selected_at: 0,
            sidebar: true,
            settings: path.as_deref().map(Settings::load).unwrap_or_default(),
            settings_path: path,
            collapsed: HashSet::new(),
            focus,
            stats: Rc::new(RefCell::new(stats)),
            wakes: (0, 0),
            echo: Default::default(),
            models: Vec::new(),
            root_seen: Default::default(),
            wake_seen: (0, 0),
            deadlines: Default::default(),
            frames: 0,
            posted: Vec::new(),
            asked: Default::default(),
            overlays: overlays::Overlays::new(cx),
            renders: 0,
            prepares: 0,
            keymap: Keymap::app(&json!({})),
            inbox: Arc::default(),
            keymap_asked: None,
            keymap_again: false,
            pending: Vec::new(),
            repeat: keymap::Repeat::default(),
            palette: None,
            recent: Vec::new(),
            activity: false,
            attention: Default::default(),
            notifier: Default::default(),
            dock: None,
            platform: false,
            native_key: || None,
            stopped: false,
            ending: false,
            relaunched: None,
            window_active: window.is_window_active(),
            font_size: look.font_size,
            theme: look.theme(dark),
            themes: theme_view::ThemeUi::new(look.theme(dark)),
            font_family: look.font_family.clone().into(),
            look,
            dark,
            config_path: None,
            settings_window: None,
            epoch: Instant::now(),
            seen_focus: None,
            term,
            floats: Default::default(),
            grids: HashMap::new(),
            grid_renders: Rc::default(),
            sections: HashMap::new(),
            section_renders: Rc::default(),
            pulse: cx.new(|_| pulse_view::Pulse::new()),
            pulse_wanted: false,
            pulse_root: None,
            hovered_pane: None,
            hover_rects: Vec::new(),
            fade: fade_view::FadeUi::new(cx.background_executor().clone()),
            layout: Default::default(),
            glass: glass_view::GlassUi::new(window.window_handle()),
            overview: Default::default(),
            updates,
            updates_auto: false,
            check_for_updates: None,
            updates_looked: None,
            shader: Default::default(),
            wake,
            _wakes: wakes,
            _clocks: clocks,
            _activation: activation,
            _bounds: bounds,
            _quit: quit,
            _release: release,
            _appearance: appearance,
            _prepare: prepare,
            _refocus: refocus,
            _lost: lost,
            ime: Default::default(),
        };
        ws.prepare(); // the first frame comes before any notify
        ws.root_seen = ws.root_input();
        ws.note_layout(window);
        ws.sync_grids(cx);
        ws.note_reduce_transparency();
        ws.note_reduce_motion();
        ws.update_glass(window, cx);
        ws.sync_sections(window, cx);
        ws
    }
}
