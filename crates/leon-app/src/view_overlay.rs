//! What floats over the panes: the command palette, and the hint for a pending key sequence, each
//! fading and scaling in and out (view_presence.rs).

use super::*;
use crate::overlay_motion::Look;
use crate::palette::Item;

/// Palette rows shown at once; the query narrows the rest.
const PALETTE_ROWS: usize = 12;

/// The palette's width.
const PALETTE_W: f32 = 520.;

/// What the palette shows, as `prepare` saw it: render draws this while it is open, and the last of
/// it while it fades out (view_presence.rs).
#[derive(Clone, PartialEq)]
pub(in crate::view) struct PaletteView {
    /// The rows in view, by their place in the matches.
    rows: Vec<(usize, Item)>,
    selected: usize,
    /// The switcher's note, an ask's row, or what says nothing matched.
    note: Option<String>,
    ask: Option<String>,
    none: Option<&'static str>,
    query: String,
    prompt: String,
}

impl Workspace {
    /// The palette as it shows now (`note_overlays`, while it is open).
    pub(super) fn palette_view(&self) -> PaletteView {
        let selected = self.palette_selected();
        let matches = self.palette_matches();
        // keep the selected row in view
        let first = selected.saturating_sub(PALETTE_ROWS - 1);
        let rows = matches.iter().cloned().enumerate().skip(first).take(PALETTE_ROWS).collect();
        let ask = self.ask_rows().map(|(_, row)| row);
        let none = (ask.is_none() && matches.is_empty()).then(|| match self.palette.as_ref() {
            _ if self.switcher_open() => "No matching sessions, tabs or blocks",
            Some(p) if p.themes.is_some() => "No matching themes",
            _ => "No matching actions",
        });
        let (query, prompt) = (self.palette_query(), self.palette_prompt());
        PaletteView { rows, selected, note: self.switcher_note(), ask, none, query, prompt }
    }

    /// The palette (open, or fading out) and a pending sequence's hint, each as it looks now.
    pub(super) fn render_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let palette = self.palette_drawn().map(|(p, look, live)| self.render_palette(p, look, live, cx));
        let hint = self.hint_drawn().map(|(h, look)| presence::faded(look, 8., render_hint(h.clone(), &self.theme)));
        if palette.is_none() && hint.is_none() {
            return None;
        }
        Some(div().absolute().size_full().children(palette).children(hint).into_any_element())
    }

    /// `p` drawn at `look`; a going palette (`live` false) takes no clicks.
    fn render_palette(&self, p: &PaletteView, look: Look, live: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let mut list = div().flex().flex_col().py(px(4.));
        for (i, item) in &p.rows {
            let (i, name, args) = (*i, item.name.clone(), item.args.clone());
            let row = div()
                .id(SharedString::from(format!("palette-{i}")))
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .h(px(28.))
                .px(px(12.))
                .mx(px(4.))
                .rounded(px(5.))
                .cursor_pointer()
                .when(i == p.selected, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                .when(i != p.selected, |d| d.text_color(rgb(t.text)))
                .child(
                    div()
                        .flex_1()
                        .overflow_hidden()
                        .min_w(px(0.))
                        .line_clamp(1)
                        .text_ellipsis()
                        .child(item.title.clone()),
                )
                .children(
                    item.shortcut
                        .clone()
                        .map(|k| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(k)),
                );
            let row = row.when(live, |r| {
                r.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, _, window, cx| {
                        ws.palette = None;
                        ws.run_action(&name, args.as_ref(), "palette", window, cx);
                        cx.stop_propagation();
                    }),
                )
            });
            list = list.child(row);
        }
        for note in p.note.iter().chain(&p.ask).cloned().chain(p.none.map(String::from)) {
            list = list.child(div().px(px(16.)).py(px(6.)).text_color(rgb(t.muted)).child(note));
        }
        let prompt = if p.query.is_empty() {
            div().text_color(rgb(t.muted)).child(p.prompt.clone())
        } else {
            div().text_color(rgb(t.text)).child(p.query.clone())
        };
        let palette = div()
            .id("palette")
            .debug_selector(|| sel("palette".into()))
            .w(px(PALETTE_W))
            .flex()
            .flex_col()
            .bg(rgb(t.chrome))
            .border_1()
            .border_color(rgb(t.border))
            .rounded(px(10.))
            .shadow_lg()
            .occlude()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .h(px(40.))
                    .px(px(14.))
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(div().text_color(rgb(t.accent)).child(Icon::Search.at(TOOLBAR_ICON, t.accent)))
                    .child(prompt),
            )
            .child(list);
        let palette = presence::presented(look, PALETTE_W, palette);
        div().absolute().top(px(TITLEBAR + 24.)).left_0().right_0().flex().justify_center().child(palette)
    }
}

/// A pending sequence (`ctrl+b`) and what may follow it, bottom right.
fn render_hint((prefix, next): (String, Vec<(String, String)>), t: &Theme) -> gpui::Div {
    let mut rows = div().flex().flex_col().gap(px(2.));
    for (key, what) in next {
        rows = rows.child(
            div()
                .flex()
                .gap(px(10.))
                .text_size(px(SECONDARY_TEXT))
                .child(div().min_w(px(48.)).text_color(rgb(t.accent)).child(key))
                .child(div().text_color(rgb(t.text)).child(what)),
        );
    }
    div()
        .absolute()
        .bottom(px(16.))
        .right(px(16.))
        .p(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .bg(rgb(t.chrome))
        .border_1()
        .border_color(rgb(t.border))
        .rounded(px(8.))
        .shadow_lg()
        .child(
            div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(format!("{prefix} ›   esc to cancel")),
        )
        .child(rows)
}
