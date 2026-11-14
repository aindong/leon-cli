//! The active window's panes, the window's `Render`, and painting a terminal's grid.

use super::*;

impl Workspace {
    /// The active window's panes, placed by `panes::layout` in the pane area: the window less the
    /// sidebar and the tab bar (so the size is known now, not a frame late).
    pub(super) fn render_panes(
        &self,
        w: &leon_core::Window,
        terms: Panes,
        window: &Window,
        stats: Rc<RefCell<Stats>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &DARK;
        let size = self.pane_area(window);
        let active = window.is_window_active();
        let font_size = self.font_size;
        let l =
            w.layout.as_ref().map(|tree| panes::layout(tree, w.zoomed.as_deref(), size, DIVIDER)).unwrap_or_default();
        let target = target(w).map(str::to_string);
        let several = l.panes.len() > 1;
        let mut out = div().flex_1().relative().bg(rgb(t.bg));
        for p in &l.panes {
            let Some((_, term)) = terms.iter().find(|(id, _)| *id == p.block_id) else { continue };
            let focused = target.as_deref() == Some(p.block_id.as_str());
            let id = p.block_id.clone();
            let caption = status::caption(&term.program_status()).map(|c| {
                let shown = status::indicator(&term.program_status());
                let color = shown.map_or(t.muted, |i| i.color(t));
                div()
                    .absolute()
                    .top(px(4.))
                    .right(px(6.))
                    .max_w(px((p.rect.w * 0.6).max(40.)))
                    .px(px(6.))
                    .rounded(px(4.))
                    .bg(gpui::Hsla::from(rgb(t.chrome)).opacity(0.9))
                    .text_xs()
                    .text_color(rgb(t.text))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .border_l_2()
                    .border_color(rgb(color))
                    .child(c)
            });
            out = out.child(
                div()
                    .id(SharedString::from(format!("pane-{}", p.block_id)))
                    .debug_selector(|| format!("pane-{}", p.block_id))
                    .absolute()
                    .left(px(p.rect.x))
                    .top(px(p.rect.y))
                    .w(px(p.rect.w))
                    .h(px(p.rect.h))
                    .p(px(PADDING - 1.))
                    .border_1()
                    .border_color(if several && focused {
                        gpui::Hsla::from(rgb(t.accent)).opacity(0.55)
                    } else {
                        gpui::transparent_black()
                    })
                    .child(terminal_canvas(term.clone(), active && focused, font_size, stats.clone()))
                    .children(caption)
                    .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.focus_block(&id, cx))),
            );
        }
        for (i, d) in l.dividers.iter().enumerate() {
            let r = d.rect;
            let across = d.direction == Direction::Horizontal;
            let (x, y, w, h) = if across {
                (r.x - DIVIDER_GRAB, r.y, r.w + 2. * DIVIDER_GRAB, r.h)
            } else {
                (r.x, r.y - DIVIDER_GRAB, r.w, r.h + 2. * DIVIDER_GRAB)
            };
            let divider = d.clone();
            let line = if across { div().w(px(DIVIDER)).h_full() } else { div().h(px(DIVIDER)).w_full() };
            out = out.child(
                div()
                    .id(SharedString::from(format!("divider-{i}")))
                    .debug_selector(|| format!("divider-{i}"))
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .flex()
                    .justify_center()
                    .items_center()
                    .occlude()
                    .cursor(if across { CursorStyle::ResizeLeftRight } else { CursorStyle::ResizeUpDown })
                    .child(line.bg(rgb(t.border)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, ev: &gpui::MouseDownEvent, _, cx| {
                            ws.start_drag(divider.clone(), ev.position, cx)
                        }),
                    ),
            );
        }
        out
    }
}

impl Workspace {
    /// The pane area's size (w, h): the window less the sidebar and the tab bar.
    pub(super) fn pane_area(&self, window: &Window) -> (f32, f32) {
        let view = window.viewport_size();
        let sidebar = if self.sidebar { SIDEBAR_WIDTH } else { 0. };
        ((f32::from(view.width) - sidebar).max(0.), (f32::from(view.height) - TAB_BAR_HEIGHT).max(0.))
    }

    /// The user arriving at a pane (another pane, tab or session selected, or the window made active)
    /// has seen its done and error: clear them (spec §5 lifetime). Not while they stay on it, so a
    /// finish in the pane they are looking at still shows until they come back.
    fn note_focus(&mut self, window: &Window) {
        let now = self.target().map(|(_, id, _)| (id, window.is_window_active()));
        let arrived = match (&self.seen_focus, &now) {
            (_, None) => false,
            (Some((was, was_active)), Some((id, active))) => was != id || (*active && !was_active),
            (None, Some(_)) => true,
        };
        self.seen_focus = now;
        if arrived {
            if let Some(t) = self.terminal() {
                t.acknowledge();
            }
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.note_focus(window);
        let stats = self.stats.clone();
        if stats.borrow().enabled {
            stats.borrow_mut().render_start = Some(Instant::now());
        }
        let rows = self.rows();
        self.settle(&rows);
        let current = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let body = match self.shown() {
            Some((w, terms)) => self.render_panes(&w, terms, window, stats, cx).into_any_element(),
            None => div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .bg(rgb(DARK.bg))
                .child(div().text_color(rgb(DARK.text)).child("No sessions"))
                .child(div().text_sm().text_color(rgb(DARK.muted)).child("Press ⌘N to start one"))
                .into_any_element(),
        };
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_pane))
            .on_action(cx.listener(Self::split_right))
            .on_action(cx.listener(Self::split_down))
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::toggle_zoom))
            .child(self.drag_listener(cx))
            .on_action(cx.listener(Self::new_session))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_key_down(cx.listener(Self::key_down))
            .children(self.render_overlay(cx))
            .size_full()
            .flex()
            .bg(rgb(DARK.bg))
            .font_family(".SystemUIFont")
            .text_color(rgb(DARK.text))
            .when(self.sidebar, |d| d.child(self.render_sidebar(&rows, cx)))
            .child(div().flex_1().flex().flex_col().min_w(px(0.)).child(self.render_tabs(current, cx)).child(body))
    }
}

/// The argv0 a tab shows: `/bin/zsh` → `zsh`, a login shell's `-zsh` → `zsh`.
pub(super) fn program_name(command: &[String]) -> String {
    let argv0 = command.first().map_or("", String::as_str);
    argv0.rsplit('/').next().unwrap_or(argv0).trim_start_matches('-').to_string()
}

pub(super) fn terminal_font(bold: bool, italic: bool) -> Font {
    Font {
        weight: if bold { FontWeight::BOLD } else { FontWeight::NORMAL },
        style: if italic { FontStyle::Italic } else { FontStyle::Normal },
        ..font(FONT_FAMILY)
    }
}

/// A terminal cell's (width, height) at `pt`.
pub(super) fn cell_size(window: &Window, pt: f32) -> (Pixels, Pixels) {
    let ts = window.text_system();
    let font_id = ts.resolve_font(&terminal_font(false, false));
    let cell_w = ts.advance(font_id, px(pt), 'm').map(|s| s.width).ok().filter(|w| *w > px(0.)).unwrap_or(px(8.));
    (cell_w, (px(pt) * LINE_HEIGHT).round())
}

/// Resize `term` to the cells that fit in (w, h), if that changes its size.
pub(super) fn fit(term: &Terminal, w: Pixels, h: Pixels, cell_w: Pixels, line_h: Pixels) {
    let cells = |len: Pixels, unit: Pixels, least: u16| ((len / unit).floor().max(0.) as u16).max(least);
    // At least what `Terminal::resize` clamps to, or every frame would resize again.
    let (rows, cols) = (cells(h, line_h, 1), cells(w, cell_w, 2));
    if term.size() != (rows, cols) {
        let _ = term.resize(rows, cols);
    }
}

/// Paints one terminal. Prepaint fits the grid to the bounds (resizing the PTY when rows or
/// columns change) and builds the render model under the terminal's lock; paint draws it.
pub(super) fn terminal_canvas(
    term: Arc<Terminal>,
    active: bool,
    font_size: f32,
    stats: Rc<RefCell<Stats>>,
) -> impl IntoElement {
    canvas(
        move |bounds, window, _| {
            let (cell_w, line_h) = cell_size(window, font_size);
            fit(&term, bounds.size.width, bounds.size.height, cell_w, line_h);
            let frame = term.with_term(|t| render::frame(t, &DARK));
            (frame, cell_w, line_h)
        },
        move |bounds, (frame, cell_w, line_h), window, cx| {
            paint_frame(&frame, &DARK, bounds, cell_w, line_h, font_size, active, window, cx);
            stats.borrow_mut().painted();
        },
    )
    .size_full()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_frame(
    f: &Frame,
    t: &Theme,
    bounds: Bounds<Pixels>,
    cell_w: Pixels,
    line_h: Pixels,
    font_size: f32,
    active: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let at =
        |line: usize, col: usize| point(bounds.origin.x + cell_w * col as f32, bounds.origin.y + line_h * line as f32);
    for r in &f.backgrounds {
        window.paint_quad(fill(Bounds::new(at(r.line, r.col), size(cell_w * r.len as f32, line_h)), rgb(r.color)));
    }
    let mark = f.cursor.as_ref().map(|c| {
        let w = cell_w * if c.wide { 2. } else { 1. };
        let origin = at(c.line, c.col);
        match c.shape {
            CursorShape::Beam => Mark::Bar(Bounds::new(origin, size(px(2.), line_h))),
            CursorShape::Underline => {
                Mark::Bar(Bounds::new(point(origin.x, origin.y + line_h - px(2.)), size(w, px(2.))))
            }
            CursorShape::Block if active => Mark::Block(Bounds::new(origin, size(w, line_h))),
            _ => Mark::Hollow(Bounds::new(origin, size(w, line_h))), // also any block in an inactive window
        }
    });
    let mut paint_text =
        |text: &str, line: usize, col: usize, color: u32, s: Option<&render::Span>, window: &mut Window| {
            let (bold, italic, underline, strike) =
                s.map_or((false, false, false, false), |s| (s.bold, s.italic, s.underline, s.strikeout));
            let run = TextRun {
                len: text.len(),
                font: terminal_font(bold, italic),
                color: rgb(color).into(),
                background_color: None,
                underline: underline.then(|| UnderlineStyle { thickness: px(1.), color: None, wavy: false }),
                strikethrough: strike.then(|| StrikethroughStyle { thickness: px(1.), color: None }),
            };
            // Pin glyph i to column i, so a fallback font's advance can't push text off the grid.
            let grid = s.filter(|s| s.cells == s.text.chars().count()).map(|_| cell_w);
            let shaped =
                window.text_system().shape_line(SharedString::from(text.to_string()), px(font_size), &[run], grid);
            let _ = shaped.paint(at(line, col), line_h, window, cx);
        };
    // A solid block cursor sits under the text; its cell's text is redrawn in the background color.
    if let Some(Mark::Block(b)) = mark {
        window.paint_quad(fill(b, rgb(t.cursor)));
    }
    for (line, spans) in f.rows.iter().enumerate() {
        for s in spans {
            paint_text(&s.text, line, s.col, s.fg, Some(s), window);
        }
    }
    match (mark, &f.cursor) {
        (Some(Mark::Block(_)), Some(c)) if !c.text.trim().is_empty() => {
            paint_text(&c.text, c.line, c.col, t.bg, None, window)
        }
        (Some(Mark::Bar(b)), _) => window.paint_quad(fill(b, rgb(t.cursor))),
        (Some(Mark::Hollow(b)), _) => window.paint_quad(outline(b, rgb(t.cursor), gpui::BorderStyle::Solid)),
        _ => {}
    }
}

/// How the cursor is drawn.
enum Mark {
    /// Filled, under the text.
    Block(Bounds<Pixels>),
    /// A beam or underline bar, over the text.
    Bar(Bounds<Pixels>),
    /// An outline: a hollow-block cursor, or the cursor of an inactive window.
    Hollow(Bounds<Pixels>),
}
