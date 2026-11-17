//! The Settings window's translucency, drawn as the main window's is (view_glass.rs): init.lua's
//! `translucency` over the theme's chrome, opaque under Reduce Transparency or in full screen.

use super::*;

impl SettingsWindow {
    /// The window backgrounds GPUI was asked for, oldest first.
    pub fn backgrounds_set(&self) -> &[Bg] {
        &self.backgrounds
    }

    /// The alpha the window's chrome is filled with (1 when opaque).
    pub fn fill_alpha(&self) -> f32 {
        self.glass.chrome_alpha()
    }

    /// Use Reduce Transparency as `on` says whatever the system does (tests); None asks the system.
    pub fn set_reduce_transparency(&mut self, on: Option<bool>, window: &mut Window, cx: &mut Context<Self>) {
        self.reduce_transparency = on;
        self.note_reduce_transparency();
        self.update_glass(window, cx);
    }

    pub(super) fn note_reduce_transparency(&mut self) {
        self.reduced = self.reduce_transparency.unwrap_or_else(crate::view::system_reduce_transparency);
    }

    /// Look again at what the window shows through, as the main window does
    /// (`Workspace::update_glass`): init.lua's translucency, unless Reduce Transparency is on or
    /// the window is in full screen. GPUI is asked only for a change.
    pub(super) fn update_glass(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(l) = &self.loaded else { return };
        let see = if self.reduced || window.is_fullscreen() { 0. } else { l.look.translucency };
        let glass = Glass::new(&self.theme(), see, 0.);
        let bg = if glass.translucent() { Bg::Blurred } else { Bg::Opaque };
        if self.backgrounds.last() != Some(&bg) {
            window.set_background_appearance(bg);
            if self.backgrounds.len() == 16 {
                self.backgrounds.remove(0);
            }
            self.backgrounds.push(bg);
            if bg == Bg::Blurred {
                crate::view::set_blur_material();
            }
        }
        if glass != self.glass {
            self.glass = glass;
            cx.notify();
        }
    }

    /// [`SettingsWindow::update_glass`] once this update is over: a read ends in a task, which
    /// has no window.
    pub(super) fn glass_later(&mut self, cx: &mut Context<Self>) {
        let (handle, this) = (self.handle, cx.entity().downgrade());
        cx.defer(move |cx| {
            let _ = handle.update(cx, |_, window, cx| this.update(cx, |sw, cx| sw.update_glass(window, cx)));
        });
    }

    /// The window's fill: the chrome, at the glass's alpha.
    pub(super) fn fill(&self, t: &Theme) -> Hsla {
        Hsla::from(rgb(t.chrome)).opacity(self.glass.chrome_alpha())
    }
}
