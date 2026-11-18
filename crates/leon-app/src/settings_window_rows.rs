//! The Settings window's controls: a switch, a stepper for a number with its range, a choice of a
//! few values, and the font row made of them.

use super::*;

pub(super) const NOTIFY_DONE: &str = "A long run finished while leon was in the background";
pub(super) const NOTIFY_BLOCKED: &str = "A program is waiting for you";
pub(super) const NOTIFY_ERROR: &str = "A program reported an error";

/// What `resume` is, under its label.
pub(super) const RESUME_NOTE: &str = "What a block restored after a restart does with its program's resume command";
/// `resume`'s values, as the window names them.
pub(super) const RESUME: [(&str, &str); 3] = [("ask", "Ask"), ("always", "Always"), ("never", "Never")];

/// A number's control: its step, its range, whether init.lua takes it whole, and how it reads.
#[derive(Clone, Copy)]
pub(super) struct Steps {
    step: f64,
    min: f64,
    max: f64,
    whole: bool,
    show: fn(f64) -> String,
    range: &'static str,
}

fn ms(v: f64) -> String {
    if v == 0. {
        "Off".into()
    } else {
        format!("{v} ms")
    }
}

fn percent(v: f64) -> String {
    if v == 0. {
        "Off".into()
    } else {
        format!("{}%", (v * 100.).round())
    }
}

fn see_through(v: f64) -> String {
    if v == 0. {
        "Opaque".into()
    } else {
        percent(v)
    }
}

fn lines(v: f64) -> String {
    format!("{v}")
}

fn points(v: f64) -> String {
    format!("{v} pt")
}

pub(super) const REPEAT: Steps =
    Steps { step: 50., min: 0., max: 10_000., whole: true, show: ms, range: "0 (off) to 10000 ms" };
pub(super) const FADE: Steps = Steps {
    step: 0.05,
    min: 0.,
    max: leon_lua::settings::FADE_MAX,
    whole: false,
    show: percent,
    range: "0 (off) to 80%",
};
pub(super) const TRANSLUCENCY: Steps = Steps {
    step: 0.05,
    min: 0.,
    max: leon_lua::settings::TRANSLUCENCY_MAX,
    whole: false,
    show: see_through,
    range: "0 (opaque) to 80%",
};
pub(super) const SCROLLBACK: Steps = Steps {
    step: 1000.,
    min: 0.,
    max: leon_core::terminal::MAX_SCROLLBACK as f64,
    whole: true,
    show: lines,
    range: "0 to 100000 lines",
};
const FONT_SIZE: Steps =
    Steps { step: 1., min: FONT_SIZES.0, max: FONT_SIZES.1, whole: false, show: points, range: "8 to 32 pt" };

impl Steps {
    /// The next step boundary up (`by` 1) or down (-1) from `v`, kept in the range; None at the
    /// range's end. Rounded, so 0.5 − 0.05 is 0.45 in init.lua, not 0.44999….
    pub(super) fn next(&self, v: f64, by: f64) -> Option<f64> {
        // the next step boundary past `v` that way: from 13.5 pt, + is 14, not 15
        let k = v / self.step;
        let n = if by > 0. { (k + 1e-9).floor() + 1. } else { (k - 1e-9).ceil() - 1. };
        let to = (n * self.step).clamp(self.min, self.max);
        let to = (to * 100.).round() / 100.;
        (to != v && (by > 0. && v < self.max || by < 0. && v > self.min)).then_some(to)
    }

    fn json(&self, v: f64) -> Value {
        if self.whole {
            json!(v as u64)
        } else {
            json!(v)
        }
    }
}

impl SettingsWindow {
    /// A row's title and, under it, a line of explanation.
    pub(super) fn label(t: &Theme, title: &str, note: &str, locked: bool) -> gpui::Div {
        // it takes what the control leaves and wraps there, so the control stays in the window
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(div().text_color(rgb(if locked { t.muted } else { t.text })).child(title.to_string()))
            .when(!note.is_empty(), |d| {
                d.child(div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(note.to_string()))
            })
    }

    pub(super) fn subtitle(t: &Theme, title: &str) -> gpui::Div {
        div().flex().flex_col().gap(px(12.)).child(
            div()
                .text_size(px(UI_TEXT + 2.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(t.title))
                .child(title.to_string()),
        )
    }

    /// A setting's read-only line under its control.
    pub(super) fn locked(t: &Theme, key: &str, row: &Option<Row>) -> Option<gpui::Div> {
        let why = row.as_ref()?.read_only.clone()?;
        let sel = format!("locked-{key}");
        Some(div().debug_selector(|| sel).text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(why))
    }

    pub(super) fn is_locked(row: &Option<Row>) -> bool {
        row.as_ref().is_none_or(|r| r.read_only.is_some())
    }

    pub(super) fn font_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let family_row = self.row("font.family");
        let family = family_row.as_ref().and_then(|r| r.value.as_str()).unwrap_or("Menlo").to_string();
        let mut fonts = self.fonts.clone();
        if !fonts.contains(&family) {
            fonts.insert(0, family.clone());
        }
        let at = fonts.iter().position(|f| *f == family).unwrap_or(0);
        let step = |by: isize| fonts[(at as isize + by).rem_euclid(fonts.len() as isize) as usize].clone();
        let (back, next) = (step(-1), step(1));
        let locked = Self::is_locked(&family_row);
        let mut control = div().flex().items_center().gap(px(8.));
        let several = fonts.len() > 1;
        if several {
            control = control.child(button(t, "font-back".into(), "‹", !locked, cx, move |sw, cx| {
                sw.choose("font.family", json!(back), cx)
            }));
        }
        control = control.child(div().w(px(140.)).flex().justify_center().font_family(family.clone()).child(family));
        if several {
            control = control.child(button(t, "font-next".into(), "›", !locked, cx, move |sw, cx| {
                sw.choose("font.family", json!(next), cx)
            }));
        }
        Self::subtitle(t, "Font")
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(Self::label(t, "Family", "", locked))
                    .child(control),
            )
            .children(Self::locked(t, "font.family", &family_row))
            .child(self.stepper(t, "font.size", "Size", FONT_SIZE, cx))
    }

    /// A switch for a true/false setting, with its label.
    pub(super) fn toggle(&self, t: &Theme, key: &'static str, label: &str, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row(key);
        let on = row.as_ref().is_some_and(|r| r.value == true);
        let locked = Self::is_locked(&row);
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{key}")))
                    .debug_selector(|| format!("toggle-{key}"))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(Self::label(t, label, "", locked))
                    .child(switch(t, on, locked))
                    .when(!locked, |d| {
                        d.cursor_pointer().on_click(cx.listener(move |sw, _, _, cx| sw.choose(key, json!(!on), cx)))
                    }),
            )
            .children(Self::locked(t, key, &row))
    }

    /// − value + for a number, with its range under the label; a button at the range's end is inert.
    pub(super) fn stepper(
        &self,
        t: &Theme,
        key: &'static str,
        label: &str,
        steps: Steps,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let row = self.row(key);
        let v = row.as_ref().and_then(|r| r.value.as_f64()).unwrap_or(0.);
        let locked = Self::is_locked(&row);
        let go = |by: f64, cx: &mut Context<Self>, mark: &'static str, dir: &str| {
            let to = steps.next(v, by).filter(|_| !locked);
            button(t, format!("step-{dir}-{key}"), mark, to.is_some(), cx, move |sw, cx| {
                if let Some(to) = to {
                    sw.choose(key, steps.json(to), cx)
                }
            })
        };
        let control = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(go(-1., cx, "−", "down"))
            .child(div().w(px(72.)).flex().justify_center().child((steps.show)(v)))
            .child(go(1., cx, "+", "up"));
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(Self::label(t, label, steps.range, locked))
                    .child(control),
            )
            .children(Self::locked(t, key, &row))
    }

    /// One of a few values, as a row of buttons.
    pub(super) fn choice(
        &self,
        t: &Theme,
        key: &'static str,
        label: &str,
        note: &str,
        values: &[(&'static str, &'static str)],
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let row = self.row(key);
        let now = row.as_ref().and_then(|r| r.value.as_str()).unwrap_or_default().to_string();
        let locked = Self::is_locked(&row);
        let mut group = div().flex().rounded(px(6.)).border_1().border_color(rgb(t.border)).overflow_hidden();
        for &(value, name) in values {
            let on = now == value;
            let sel = format!("choice-{key}-{value}");
            group = group.child(
                div()
                    .id(SharedString::from(sel.clone()))
                    .debug_selector(|| sel)
                    .px(px(12.))
                    .py(px(4.))
                    .text_color(rgb(if on {
                        t.title
                    } else if locked {
                        t.muted
                    } else {
                        t.text
                    }))
                    .when(on, |d| d.bg(rgb(t.selected)))
                    .when(!on && !locked, |d| {
                        d.cursor_pointer()
                            .hover(|d| d.bg(rgb(t.hover)))
                            .on_click(cx.listener(move |sw, _, _, cx| sw.choose(key, json!(value), cx)))
                    })
                    .child(name),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(Self::label(t, label, note, locked))
                    .child(group),
            )
            .children(Self::locked(t, key, &row))
    }
}

/// A small square button with a label; inert when not `enabled`.
pub(super) fn button(
    t: &Theme,
    id: String,
    label: &'static str,
    enabled: bool,
    cx: &mut Context<SettingsWindow>,
    run: impl Fn(&mut SettingsWindow, &mut Context<SettingsWindow>) + 'static,
) -> impl IntoElement {
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(|| id)
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(t.border))
        .text_color(rgb(if enabled { t.title } else { t.muted }))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))).on_click(cx.listener(move |sw, _, _, cx| run(sw, cx)))
        })
        .child(label)
}

/// An on/off switch.
fn switch(t: &Theme, on: bool, locked: bool) -> impl IntoElement {
    let track = if on { t.accent } else { t.border };
    // the lighter of the pane and title colours, so the knob is light on either theme
    let knob = if t.is_dark() { t.title } else { t.bg };
    div()
        .w(px(34.))
        .h(px(20.))
        .flex_none()
        .rounded_full()
        .p(px(2.))
        .flex()
        .when(on, |d| d.justify_end())
        .bg(rgb(track))
        .when(locked, |d| d.opacity(0.5))
        .child(div().size(px(16.)).rounded_full().bg(rgb(knob)))
}
