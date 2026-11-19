//! The workspace's side of cursor shaders (docs/architecture/cursor-shaders.md): the file
//! init.lua names, loaded on GPUI's background executor when the setting or the file changes;
//! the window's overlay given the compiled shader, turned on only while it should draw; the
//! focused pane's cursor handed to it; and a failure shown in the notice. The overlay itself is
//! `crate::cursor_shader`.

use std::path::Path;
use std::time::SystemTime;

use gpui::Task;

use super::*;
use crate::cursor_shader::{self as shader, Overlay, Program};
use crate::updates::{Act, Notice};

/// How often the shader file is looked at for a change while one is set: a stat on the
/// background executor, which wakes nothing unless the file changed.
const WATCH: Duration = Duration::from_secs(1);

/// A file's modification time and length: it changed when this did.
type Stamp = Option<(SystemTime, u64)>;

fn stamp_of(path: &Path) -> Stamp {
    std::fs::metadata(path).ok().map(|m| (m.modified().unwrap_or(SystemTime::UNIX_EPOCH), m.len()))
}

/// What a load gave: the compiled shader (None on GPUI's test platform, which has no Metal
/// layer to draw it on), or why it failed.
type Loaded = Result<Option<Arc<Program>>, String>;

#[derive(Default)]
pub(super) struct ShaderUi {
    /// The file init.lua names, resolved; None: no shader.
    path: Option<PathBuf>,
    duration: Duration,
    /// The file as it was when it was last loaded.
    stamp: Stamp,
    /// Load it (again) on the next sync.
    want: bool,
    loading: bool,
    /// What the last load gave, for the next sync (which has the window).
    loaded: Option<Loaded>,
    /// The window's overlay, once a shader loaded; the grids feed it.
    pub(super) overlay: Option<Arc<Overlay>>,
    /// What the overlay was last told: on or off, and the duration and theme it draws with.
    told: Option<bool>,
    configured: Option<(Duration, Theme)>,
    /// The last load's failure, shown in the notice until dismissed.
    error: Option<String>,
    dismissed: bool,
    watch: Option<Task<()>>,
}

impl Workspace {
    /// init.lua's `cursor_shader` and `cursor_shader_duration` from a `server.settings` reply: a
    /// new file is loaded on the next sync.
    pub(super) fn note_cursor_shader(&mut self, settings: &Value) {
        let d = leon_lua::Settings::default();
        let (lo, hi) = leon_lua::settings::CURSOR_SHADER_DURATIONS;
        let secs = settings["cursor_shader_duration"].as_f64().unwrap_or(d.cursor_shader_duration).clamp(lo, hi);
        self.shader.duration = Duration::from_secs_f64(secs);
        let setting = settings["cursor_shader"].as_str().unwrap_or_default();
        let home = leon_core::paths::home();
        let path =
            self.config_path().and_then(|config| leon_lua::settings::shader_path(setting, &config, home.as_deref()));
        if path != self.shader.path {
            self.shader.path = path;
            self.shader.watch = None; // the next sync watches the new one
            self.shader.stamp = None;
            self.shader.want = self.shader.path.is_some();
            self.shader.error = None;
        }
    }

    /// Bring the window's overlay in line, after `prepare` (it has the window): load a shader
    /// that is wanted, attach what loaded, take it away when the setting is cleared, and turn it
    /// on only while the window is active, Reduce Motion is off, and the overview and palette
    /// are closed.
    pub(super) fn sync_cursor_shader(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.shader.want && !self.shader.loading {
            self.load_cursor_shader(cx);
        }
        if self.shader.path.is_some() && self.shader.watch.is_none() {
            self.shader.watch = Some(self.watch_cursor_shader(cx));
        }
        let loaded = self.shader.loaded.take();
        if loaded.is_some() {
            // `prepare` read the notice and the grids' inputs before this: another pass shows them
            cx.notify();
        }
        match loaded {
            Some(Ok(program)) => {
                let overlay = self.shader.overlay.get_or_insert_with(Overlay::new).clone();
                // GPUI's test platform has no layer to draw on: the overlay keeps the cursor only
                let drew = program.map_or(Ok(()), |p| overlay.attach(window, p));
                match drew {
                    Ok(()) => self.shader.error = None,
                    Err(e) => self.cursor_shader_failed(format!("can't draw over the window: {e}")),
                }
            }
            Some(Err(e)) => self.cursor_shader_failed(e), // the last good shader keeps drawing
            None => {}
        }
        if self.shader.path.is_none() && self.shader.overlay.is_some() {
            self.drop_cursor_shader();
            // a load still running keeps its flag, so a new path doesn't start a second one
            self.shader = ShaderUi { loading: self.shader.loading, ..Default::default() };
        }
        let Some(overlay) = self.shader.overlay.clone() else { return };
        let look = (self.shader.duration, self.theme);
        if self.shader.configured != Some(look) {
            overlay.configure(look.0, &look.1);
            self.shader.configured = Some(look);
        }
        let on = self.window_active && !self.reduce_motion() && !self.overview_shown() && !self.palette_open();
        if self.shader.told != Some(on) {
            overlay.enable(on);
            self.shader.told = Some(on);
        }
    }

    /// The pane whose cursor the overlay draws: the focused pane of the active window, unless it
    /// is a tile under a shown float (the overlay draws over every float, so only the top one).
    pub fn cursor_feeder(&self) -> Option<String> {
        let (_, id, _) = self.target().filter(|_| self.window_active)?;
        let w = self.active_window()?;
        let o = self.overlays_of(&w);
        let under = o.floating.iter().rev().find(|f| !f.hidden).is_some_and(|top| top.block_id != id);
        (!under).then_some(id)
    }

    /// Hand the overlay the cursor the feeding pane's grid draws: after a pushed screen brought
    /// that grid up to date (its model event, view_feed.rs) and after each drawn frame (where the
    /// grid now sits: a slide, a tab switch), never from paint. Placed where the grid last
    /// painted (its hit-test record); a grid not painted yet feeds nothing until it is.
    pub(super) fn feed_cursor(&self, window: &Window) {
        let Some(overlay) = &self.shader.overlay else { return };
        let Some(id) = self.cursor_feeder() else { return };
        let Some((frame, active, rgb)) = self.grid_given(&id) else { return };
        let Some(p) = self.term.painted.borrow().get(&id).copied() else { return };
        let drawn = cursor_drawn(frame, p.origin, p.cell, active).map(|(b, style, cell)| shader::Drawn {
            block: id.clone(),
            rect: [b.origin.x, b.origin.y, b.size.width, b.size.height].map(f32::from),
            scale: window.scale_factor(),
            style,
            cell,
            rgb,
        });
        overlay.feed(window, drawn.as_ref());
    }

    /// The window's size or display changed: the overlay follows (resize, full screen, Retina).
    pub(super) fn fit_cursor_shader(&self, window: &Window) {
        if let Some(o) = &self.shader.overlay {
            o.fit(window);
        }
    }

    /// The window closes: its overlay's layer and link go.
    pub(super) fn drop_cursor_shader(&mut self) {
        if let Some(o) = self.shader.overlay.take() {
            o.detach();
        }
    }

    fn cursor_shader_failed(&mut self, why: String) {
        eprintln!("leon: cursor shader: {why}");
        self.shader.error = Some(why);
        self.shader.dismissed = false;
    }

    /// Read, translate and (in the app) compile the file on the background executor.
    fn load_cursor_shader(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.shader.path.clone() else { return };
        (self.shader.want, self.shader.loading) = (false, true);
        let compiles = self.platform;
        let task = cx.background_executor().spawn(async move {
            let stamp = stamp_of(&path);
            let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let loaded = shader::load(&path).and_then(|msl| match compiles {
                true => shader::compile(&msl).map(Some).map_err(|e| format!("{name}: {e}")),
                false => Ok(None),
            });
            (path, stamp, loaded)
        });
        cx.spawn(async move |this, cx| {
            let (path, stamp, loaded) = task.await;
            this.update(cx, |ws, cx| {
                ws.shader.loading = false;
                // the setting moved on while this loaded: the next sync loads the new one
                if ws.shader.path.as_ref() == Some(&path) {
                    (ws.shader.stamp, ws.shader.loaded) = (stamp, Some(loaded));
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Look at the file every `WATCH` while one is set; a change loads it again.
    fn watch_cursor_shader(&self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(WATCH).await;
            let Ok(Some((path, was))) = this.update(cx, |ws, _| ws.shader.path.clone().map(|p| (p, ws.shader.stamp)))
            else {
                return;
            };
            let now = cx.background_executor().spawn(async move { stamp_of(&path) }).await;
            if now != was {
                let more = this.update(cx, |ws, cx| {
                    if !ws.shader.loading {
                        ws.shader.want = true;
                        cx.notify();
                    }
                });
                if more.is_err() {
                    return;
                }
            }
        })
    }

    /// The notice for a shader that failed to load, until dismissed.
    pub(super) fn cursor_shader_notice(&self) -> Option<Notice> {
        let why = self.shader.error.as_ref().filter(|_| !self.shader.dismissed)?;
        Some(Notice { text: format!("Cursor shader: {why}"), actions: vec![Act::Dismiss] })
    }

    /// The notice to show: the update's, else the cursor shader's failure.
    pub(super) fn notice(&self) -> Option<Notice> {
        self.updates.notice().or_else(|| self.cursor_shader_notice())
    }

    /// Dismiss the notice shown: the update's, else the cursor shader's.
    pub(super) fn dismiss_notice(&mut self) {
        match self.updates.notice() {
            Some(_) => self.updates.dismiss(),
            None => self.shader.dismissed = true,
        }
    }

    /// The window's overlay's cursor state (tests).
    pub fn cursor_shader_state(&self) -> Option<shader::Cursor> {
        self.shader.overlay.as_ref().map(|o| o.state())
    }

    /// The failure the notice shows, dismissed or not (tests).
    pub fn cursor_shader_error(&self) -> Option<&str> {
        self.shader.error.as_deref()
    }
}
