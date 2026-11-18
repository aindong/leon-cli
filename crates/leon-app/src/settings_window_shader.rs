//! The cursor shader's rows under Appearance (docs/cursor-shaders.md): the file, with Choose… (an
//! open panel) and None, and how long it draws after a move.

use super::rows::Steps;
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

/// A button with a word on it.
fn text_button(
    t: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
    cx: &mut Context<SettingsWindow>,
    run: impl Fn(&mut SettingsWindow, &mut Context<SettingsWindow>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .px(px(10.))
        .py(px(3.))
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(t.border))
        .text_color(rgb(if enabled { t.title } else { t.muted }))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))).on_click(cx.listener(move |sw, _, _, cx| run(sw, cx)))
        })
        .child(label)
}

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
            .child(text_button(t, "cursor-shader-choose", "Choose…", !locked, cx, |sw, cx| {
                sw.choose_cursor_shader(cx)
            }))
            .child(text_button(t, "cursor-shader-none", "None", !locked && !file.is_empty(), cx, |sw, cx| {
                sw.choose("cursor_shader", json!(""), cx)
            }));
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
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let rest = home.as_deref().and_then(|h| path.strip_prefix(h).ok()).filter(|r| !r.as_os_str().is_empty());
        let text = rest.map_or_else(|| path.display().to_string(), |r| format!("~/{}", r.display()));
        self.choose("cursor_shader", json!(text), cx);
    }
}
