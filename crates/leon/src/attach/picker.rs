//! The picker: every session, its tabs and their blocks, filtered by typing (the command
//! palette's fuzzy match), with the selected row's block previewed live.

use std::collections::HashMap;
use std::sync::Arc;

use leon_app::palette::{filter, Item};
use leon_app::remote::Session;
use leon_core::screen::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use super::draw::{self, brand, Colors};

/// The heading; tests look for it to know the picker is showing.
pub const HEADING: &str = "Pick a session, tab or block";

/// What a row attaches to.
#[derive(Clone, Debug, PartialEq)]
pub enum Pick {
    Session(String),
    Window(String, String),
    /// (session, block): the block's tab opens with it focused.
    Block(String, String),
    /// No sessions: start one.
    New,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub depth: u16,
    pub text: String,
    /// "session › tab › block", what the filter matches.
    pub path: String,
    pub pick: Pick,
    /// The block the preview shows.
    pub block: Option<String>,
}

/// A session's name as the app's sidebar shows it: its label, else "Session N" (1-based).
pub fn session_name(s: &Session, index: usize) -> String {
    s.label.clone().unwrap_or_else(|| format!("Session {}", index + 1))
}

/// A tab's name as the app's tab bar shows it: its label, else the shown block's program.
pub fn tab_name(s: &Session, w: &leon_core::Window) -> String {
    let shown = super::overlays::focused(w).and_then(|id| s.blocks.iter().find(|b| b.id == id));
    w.label.clone().unwrap_or_else(|| shown.map_or("empty".into(), |b| program_name(&b.command)))
}

/// argv0's file name without a login shell's `-` (as the app's `program_name`, which is private).
fn program_name(command: &[String]) -> String {
    let argv0 = command.first().map_or("", String::as_str);
    argv0.rsplit('/').next().unwrap_or(argv0).trim_start_matches('-').to_string()
}

/// A block row: its program and arguments, so the filter can find a command.
fn program(b: &leon_app::remote::Block) -> String {
    let mut words = vec![program_name(&b.command)];
    words.extend(b.command.iter().skip(1).cloned());
    words.join(" ")
}

pub fn rows(sessions: &[Session]) -> Vec<Row> {
    let mut out = Vec::new();
    for (i, s) in sessions.iter().enumerate() {
        let name = session_name(s, i);
        let active = s.windows.get(s.active);
        out.push(Row {
            depth: 0,
            text: name.clone(),
            path: name.clone(),
            pick: Pick::Session(s.id.clone()),
            block: active.and_then(super::overlays::focused).map(str::to_string),
        });
        for (i, w) in s.windows.iter().enumerate() {
            let tab = format!("{}:{}", i + 1, tab_name(s, w));
            out.push(Row {
                depth: 1,
                text: tab.clone(),
                path: format!("{name} › {tab}"),
                pick: Pick::Window(s.id.clone(), w.id.clone()),
                block: super::overlays::focused(w).map(str::to_string),
            });
            let mut ids = w.layout.as_ref().map(|l| l.leaves()).unwrap_or_default();
            for b in s.blocks.iter().filter(|b| b.window_id == w.id) {
                if !ids.contains(&b.id.as_str()) {
                    ids.push(&b.id);
                }
            }
            for leaf in ids {
                let Some(b) = s.blocks.iter().find(|b| b.id == leaf) else { continue };
                let text = program(b);
                out.push(Row {
                    depth: 2,
                    path: format!("{name} › {tab} › {text}"),
                    text,
                    pick: Pick::Block(s.id.clone(), b.id.clone()),
                    block: Some(b.id.clone()),
                });
            }
        }
    }
    out
}

pub enum Picked {
    Stay,
    Attach(Pick),
    /// Esc: back to the session it came from, or quit.
    Leave,
}

#[derive(Default)]
pub struct Picker {
    pub query: String,
    pub selected: usize,
}

impl Picker {
    /// The rows the query keeps, best match first (all of them, in order, for no query).
    pub fn shown(&self, rows: &[Row]) -> Vec<usize> {
        let items: Vec<Item> = rows
            .iter()
            .map(|r| Item {
                name: String::new(),
                title: r.path.clone(),
                keywords: Vec::new(),
                shortcut: None,
                args: None,
            })
            .collect();
        filter(&items, &self.query)
    }

    /// The selected index, kept on a row when rows went away (a session closed).
    fn at(&self, shown: usize) -> usize {
        self.selected.min(shown.saturating_sub(1))
    }

    pub fn selected_row<'a>(&self, rows: &'a [Row]) -> Option<&'a Row> {
        let shown = self.shown(rows);
        shown.get(self.at(shown.len())).map(|&i| &rows[i])
    }

    pub fn key(&mut self, k: &KeyEvent, rows: &[Row]) -> Picked {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let n = self.shown(rows).len();
        self.selected = self.at(n);
        match k.code {
            KeyCode::Esc => return Picked::Leave,
            KeyCode::Char('c') if ctrl => return Picked::Leave,
            KeyCode::Enter if rows.is_empty() => return Picked::Attach(Pick::New),
            KeyCode::Enter => return self.selected_row(rows).map_or(Picked::Stay, |r| Picked::Attach(r.pick.clone())),
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Char('p') if ctrl => self.selected = self.selected.saturating_sub(1),
            KeyCode::Down => self.selected = (self.selected + 1).min(n.saturating_sub(1)),
            KeyCode::Char('n') if ctrl => self.selected = (self.selected + 1).min(n.saturating_sub(1)),
            KeyCode::Backspace => {
                self.query.pop();
                self.selected = 0;
            }
            KeyCode::Char(c) if !ctrl && !k.modifiers.contains(KeyModifiers::ALT) => {
                self.query.push(c);
                self.selected = 0;
            }
            _ => {}
        }
        Picked::Stay
    }

    /// The heading, the query, the rows on the left and the selected block's screen on the right.
    pub fn draw(
        &self,
        buf: &mut Buffer,
        area: Rect,
        rows: &[Row],
        screens: &HashMap<String, Arc<Frame>>,
        colors: Colors,
    ) {
        if area.height < 4 || area.width < 10 {
            return;
        }
        let base = Style::default().fg(colors.rgb(brand::CHALK));
        let head = Rect { height: 1, ..area };
        buf.set_style(head, Style::default().bg(colors.rgb(brand::IRON)));
        let badge =
            Style::default().bg(colors.rgb(brand::MAROON)).fg(colors.rgb(brand::AMBER)).add_modifier(Modifier::BOLD);
        let x = buf.set_stringn(area.x, area.y, " leon ", area.width as usize, badge).0;
        buf.set_stringn(
            x,
            area.y,
            format!(" {HEADING}"),
            area.width.saturating_sub(x) as usize,
            base.bg(colors.rgb(brand::IRON)),
        );
        let prompt = Style::default().fg(colors.rgb(brand::AMBER)).add_modifier(Modifier::BOLD);
        let x = buf.set_stringn(area.x, area.y + 1, "› ", area.width as usize, prompt).0;
        buf.set_stringn(x, area.y + 1, &self.query, (area.width - 2) as usize, Style::default());
        let hint = "type to filter · ↑↓ move · enter attach · esc back";
        let bottom = area.y + area.height - 1;
        buf.set_stringn(area.x, bottom, hint, area.width as usize, base.fg(colors.rgb(brand::UMBER)));

        let list =
            Rect { x: area.x, y: area.y + 2, width: (area.width * 2 / 5).clamp(10, 48), height: area.height - 3 };
        let shown = self.shown(rows);
        if rows.is_empty() {
            buf.set_stringn(list.x, list.y, "No sessions. Enter starts one.", list.width as usize, base);
        }
        // keep the selected row in view
        let sel = self.at(shown.len());
        let skip = (sel + 1).saturating_sub(list.height as usize);
        for (n, &i) in shown.iter().enumerate().skip(skip).take(list.height as usize) {
            let r = &rows[i];
            let y = list.y + (n - skip) as u16;
            let indent = if self.query.is_empty() { r.depth * 2 } else { 0 };
            let text = if self.query.is_empty() { r.text.clone() } else { r.path.clone() };
            let mut style = if r.depth == 0 { base.add_modifier(Modifier::BOLD) } else { base };
            if n == sel {
                style = Style::default()
                    .bg(colors.rgb(brand::AMBER))
                    .fg(colors.rgb(brand::IRON))
                    .add_modifier(Modifier::BOLD);
                buf.set_style(Rect { y, height: 1, ..list }, style);
            }
            buf.set_stringn(list.x + indent, y, text, list.width.saturating_sub(indent) as usize, style);
        }
        let preview = Rect {
            x: list.x + list.width + 1,
            y: list.y,
            width: area.width.saturating_sub(list.width + 1),
            height: list.height,
        };
        if preview.width == 0 {
            return; // too narrow for a preview
        }
        let line = Style::default().fg(colors.rgb(brand::UMBER));
        for y in preview.y..preview.y + preview.height {
            buf[(preview.x - 1, y)].set_symbol("│").set_style(line);
        }
        let block = self.selected_row(rows).and_then(|r| r.block.as_ref());
        if let Some(f) = block.and_then(|b| screens.get(b)) {
            draw::screen(buf, preview, f, colors);
        }
    }
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
