//! The Settings window's font and toggle rows, and the controls they are made of.

use super::*;

/// The notification kinds, as the window names them.
const NOTIFICATIONS: [(&str, &str); 3] = [
    ("notifications.done", "A long run finished while leon was in the background"),
    ("notifications.blocked", "A program is waiting for you"),
    ("notifications.error", "A program reported an error"),
];

impl SettingsWindow {
    pub(super) fn section(t: &Theme, title: &str) -> gpui::Div {
        div().flex().flex_col().gap(px(12.)).child(
            div()
                .text_size(px(UI_TEXT + 2.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(t.title))
                .child(title.to_string()),
        )
    }

    /// A setting's read-only line under its control.
    pub(super) fn locked(t: &Theme, row: &Option<Row>) -> Option<gpui::Div> {
        let why = row.as_ref()?.read_only.clone()?;
        Some(div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(why))
    }

    pub(super) fn font_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let family_row = self.row("font.family");
        let size_row = self.row("font.size");
        let family = family_row.as_ref().and_then(|r| r.value.as_str()).unwrap_or("Menlo").to_string();
        let pt = size_row.as_ref().and_then(|r| r.value.as_f64()).unwrap_or(13.);
        let mut fonts = self.fonts.clone();
        if !fonts.contains(&family) {
            fonts.insert(0, family.clone());
        }
        let at = fonts.iter().position(|f| *f == family).unwrap_or(0);
        let step = |by: isize| fonts[(at as isize + by).rem_euclid(fonts.len() as isize) as usize].clone();
        let family_locked = family_row.as_ref().is_some_and(|r| r.read_only.is_some()) || fonts.len() < 2;
        let size_locked = size_row.as_ref().is_some_and(|r| r.read_only.is_some());
        let (back, next) = (step(-1), step(1));
        let smaller = (pt - 1.).max(FONT_SIZES.0);
        let bigger = (pt + 1.).min(FONT_SIZES.1);
        let label = |text: &str| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(text.to_string());
        Self::section(t, "Font")
            .child(
                div()
                    .flex()
                    .gap(px(32.))
                    .child(
                        div().flex().flex_col().gap(px(6.)).child(label("Family")).child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(button(t, "font-back", "‹", !family_locked, cx, move |sw, cx| {
                                    sw.choose("font.family", json!(back), cx)
                                }))
                                .child(
                                    div().w(px(140.)).flex().justify_center().font_family(family.clone()).child(family),
                                )
                                .child(button(t, "font-next", "›", !family_locked, cx, move |sw, cx| {
                                    sw.choose("font.family", json!(next), cx)
                                })),
                        ),
                    )
                    .child(
                        div().flex().flex_col().gap(px(6.)).child(label("Size")).child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(button(
                                    t,
                                    "size-down",
                                    "−",
                                    !size_locked && pt > FONT_SIZES.0,
                                    cx,
                                    move |sw, cx| sw.choose("font.size", json!(smaller), cx),
                                ))
                                .child(div().w(px(52.)).flex().justify_center().child(format!("{pt} pt")))
                                .child(button(
                                    t,
                                    "size-up",
                                    "+",
                                    !size_locked && pt < FONT_SIZES.1,
                                    cx,
                                    move |sw, cx| sw.choose("font.size", json!(bigger), cx),
                                )),
                        ),
                    ),
            )
            .children(Self::locked(t, &family_row))
            .children(Self::locked(t, &size_row))
    }

    pub(super) fn toggles_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut notes = Self::section(t, "Notify me when");
        for (key, label) in NOTIFICATIONS {
            notes = notes.child(self.toggle(t, key, label, cx));
        }
        div().flex().flex_col().gap(px(28.)).child(notes).child(Self::section(t, "Selection").child(self.toggle(
            t,
            "copy_on_select",
            "Copy text as I select it",
            cx,
        )))
    }

    /// A switch for a true/false setting, with its label.
    pub(super) fn toggle(&self, t: &Theme, key: &'static str, label: &str, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row(key);
        let on = row.as_ref().is_some_and(|r| r.value == true);
        let locked = row.as_ref().is_none_or(|r| r.read_only.is_some());
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{key}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().text_color(rgb(if locked { t.muted } else { t.text })).child(label.to_string()))
                    .child(switch(t, on, locked))
                    .when(!locked, |d| {
                        d.cursor_pointer().on_click(cx.listener(move |sw, _, _, cx| sw.choose(key, json!(!on), cx)))
                    }),
            )
            .children(Self::locked(t, &row))
    }
}

/// A small square button with a label; inert when not `enabled`.
fn button(
    t: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
    cx: &mut Context<SettingsWindow>,
    run: impl Fn(&mut SettingsWindow, &mut Context<SettingsWindow>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
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
