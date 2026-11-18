//! The cursor shader's rows under Appearance (docs/cursor-shaders.md): the file, with Choose… (an
//! open panel) and None, and how long it draws after a move.

use super::rows::Steps;
use super::server::text_button;
use super::*;

fn seconds(v: f64) -> String {
    format!("{v} s")
}

const DURATION: Steps = Steps {
    step: 0.1,
    min: leon_lua::settings::CURSOR_SHADER_DURATIONS.0,
    max: leon_lua::settings::CURSOR_SHADER_DURATIONS.1,
    whole: false,
    show: seconds,
    range: "How long it draws after the cursor moves, 0.1 to 5 s",
};

impl SettingsWindow {
    pub(super) fn cursor_shader_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("cursor_shader");
        let file = row.as_ref().and_then(|r| r.value.as_str()).unwrap_or_default().to_string();
        let locked = Self::is_locked(&row);
        let shown = match Path::new(&file).file_name() {
            Some(name) if !file.is_empty() => name.to_string_lossy().into_owned(),
            _ => "None".into(),
        };
        let control = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().debug_selector(|| "cursor-shader-file".into()).child(shown))
            .when(!locked, |d| {
                d.child(text_button(t, "cursor-shader-choose".into(), "Choose…", cx, |sw, _, cx| {
                    sw.choose_cursor_shader(cx)
                }))
                .when(!file.is_empty(), |d| {
                    d.child(text_button(t, "cursor-shader-none".into(), "None", cx, |sw, _, cx| {
                        sw.choose("cursor_shader", json!(""), cx)
                    }))
                })
            });
        let note = "A Shadertoy-style GLSL file drawn over the window as the cursor moves (macOS)";
        Self::subtitle(t, "Cursor shader")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(Self::label(t, "File", note, locked))
                    .child(control),
            )
            .children(Self::locked(t, "cursor_shader", &row))
            .child(self.stepper(t, "cursor_shader_duration", "Duration", DURATION, cx))
    }

    /// Choose…: macOS's open panel, then the file picked.
    fn choose_cursor_shader(&mut self, cx: &mut Context<Self>) {
        let options = gpui::PathPromptOptions { files: true, directories: false, multiple: false, prompt: None };
        let picked = cx.prompt_for_paths(options);
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            this.update(cx, |sw, cx| sw.pick_cursor_shader(&path, cx)).ok();
        })
        .detach();
    }

    /// Set `cursor_shader` to `path`, written `~/…` when it is in the home folder.
    pub fn pick_cursor_shader(&mut self, path: &Path, cx: &mut Context<Self>) {
        let text = home_relative(path, leon_core::paths::home().as_deref());
        self.choose("cursor_shader", json!(text), cx);
    }
}
