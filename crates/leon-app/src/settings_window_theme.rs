//! The Settings window's theme picker: a column per macOS appearance, each with a preview of its
//! theme and every theme as a row with its colours.

use super::*;

impl SettingsWindow {
    pub(super) fn theme_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("theme");
        let chosen = |side: &str| row.as_ref().and_then(|r| r.value[side].as_str()).unwrap_or_default().to_string();
        let (dark, light) = (chosen("dark"), chosen("light"));
        let locked = row.as_ref().is_some_and(|r| r.read_only.is_some());
        let column = |side: &'static str, title: &str, cx: &mut Context<Self>| {
            let name = if side == "dark" { &dark } else { &light };
            let themes = &self.loaded.as_ref().expect("loaded").themes;
            let shown = themes.iter().find(|(n, _)| n == name).and_then(|(_, p)| *p);
            let mut list = div().flex().flex_col().gap(px(2.));
            for (n, p) in themes {
                let on = n == name;
                let pair = if side == "dark" { (n.clone(), light.clone()) } else { (dark.clone(), n.clone()) };
                list = list.child(
                    div()
                        .id(SharedString::from(format!("theme-{side}-{n}")))
                        .debug_selector(|| format!("theme-{side}-{n}"))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .when(on, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                        .when(!on && !locked && p.is_some(), |d| {
                            d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))).on_click(cx.listener(
                                move |sw, _, _, cx| sw.choose("theme", json!({ "dark": pair.0, "light": pair.1 }), cx),
                            ))
                        })
                        .child(dots(p.as_ref()))
                        .child(div().flex_1().child(n.clone()))
                        .when(on, |d| d.child(Icon::Done.at(ROW_ICON, t.accent)))
                        .when(p.is_none(), |d| d.child(div().text_color(rgb(t.error)).child("won't load"))),
                );
            }
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(div().text_color(rgb(t.text)).child(title.to_string()))
                .child(preview(shown.as_ref()))
                .child(list)
        };
        Self::subtitle(t, "Theme")
            .child(div().flex().gap(px(20.)).child(column("dark", "When macOS is dark", cx)).child(column(
                "light",
                "When macOS is light",
                cx,
            )))
            .children(Self::locked(t, "theme", &row))
    }
}

/// A theme's ANSI 1–6 as dots, for its row in the list.
fn dots(p: Option<&Theme>) -> impl IntoElement {
    let mut row = div().flex().gap(px(2.)).p(px(3.)).rounded(px(4.));
    if let Some(p) = p {
        row = row.bg(rgb(p.bg));
        for c in &p.ansi[1..7] {
            row = row.child(div().size(px(7.)).rounded_full().bg(rgb(*c)));
        }
    }
    row.min_w(px(57.))
}

/// A small picture of a theme: its chrome beside a pane with a prompt, a command's coloured output
/// and the cursor.
fn preview(p: Option<&Theme>) -> impl IntoElement {
    let Some(p) = p else { return div().h(px(132.)) };
    let line = |parts: Vec<(&'static str, u32)>| {
        parts.into_iter().fold(div().flex(), |d, (text, c)| d.child(div().text_color(rgb(c)).child(text)))
    };
    div()
        .h(px(132.))
        .flex()
        .rounded(px(8.))
        .overflow_hidden()
        .border_1()
        .border_color(rgb(p.border))
        .font_family("Menlo")
        .text_size(px(11.))
        .child(
            div()
                .w(px(56.))
                .bg(rgb(p.chrome))
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(8.))
                .child(div().h(px(6.)).w(px(32.)).rounded(px(3.)).bg(rgb(p.muted)))
                .child(div().h(px(6.)).w(px(24.)).rounded(px(3.)).bg(rgb(p.selected)))
                .child(div().flex().gap(px(4.)).child(div().size(px(6.)).rounded_full().bg(rgb(p.working)))),
        )
        .child(
            div()
                .flex_1()
                .bg(rgb(p.bg))
                .p(px(10.))
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(line(vec![("~/src ", p.ansi[4]), ("% ", p.fg), ("cargo test", p.fg)]))
                .child(line(vec![("   Compiling ", p.ansi[2]), ("leon", p.fg)]))
                .child(line(vec![("warning", p.ansi[3]), (": unused import", p.fg)]))
                .child(line(vec![("error", p.ansi[1]), (": 1 test failed", p.fg)]))
                .child(
                    line(vec![("~/src ", p.ansi[4]), ("% ", p.fg)]).child(div().w(px(7.)).h(px(13.)).bg(rgb(p.cursor))),
                ),
        )
}
