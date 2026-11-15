//! The session switcher (⌘K, `session.switch`): every tab on every machine as `host › session ›
//! tab` with its status, and a block when the query matches its title; the most recently used
//! tab first, the palette's fuzzy filter over the rest.

use leon_core::program_status::strip_hidden;
use leon_core::Window;

use crate::palette::{self, Item};
use crate::remote::{Model, Session};
use crate::status::{self, Indicator};

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Which machine (`Hosts` index; 0 is This Mac), and its label.
    pub host: usize,
    pub host_label: String,
    pub session_id: String,
    pub session: String,
    pub window_id: String,
    pub tab: String,
    /// A block row: the block and the title its program set.
    pub block_id: Option<String>,
    pub block: Option<String>,
    pub status: Option<Indicator>,
}

impl Row {
    /// `host › session › tab`, then ` › block` for a block row.
    pub fn text(&self) -> String {
        let place = format!("{} › {} › {}", self.host_label, self.session, self.tab);
        match &self.block {
            Some(b) => format!("{place} › {b}"),
            None => place,
        }
    }
}

/// argv0's file name: `/bin/zsh` → `zsh`, a login shell's `-zsh` → `zsh`.
pub fn program_name(command: &[String]) -> String {
    let argv0 = command.first().map_or("", String::as_str);
    // a remote server's argv goes on screen: nothing that reorders or hides text
    strip_hidden(argv0.rsplit('/').next().unwrap_or(argv0).trim_start_matches('-'))
}

/// A session's name: its label, else its place on its machine (`Session 2`).
pub fn session_title(s: &Session, index: usize) -> String {
    s.label.clone().unwrap_or_else(|| format!("Session {}", index + 1))
}

/// A tab's name: its label, else the title the shown block's program set, else that program.
pub fn tab_title(s: &Session, w: &Window) -> String {
    w.label.clone().unwrap_or_else(|| {
        let shown = w.zoomed.as_deref().or(w.focus.as_deref()).and_then(|id| s.blocks.iter().find(|b| b.id == id));
        shown.map_or("empty".into(), |b| b.title.clone().unwrap_or_else(|| program_name(&b.command)))
    })
}

/// Every tab of every machine, in order, and a row for each block with a title.
pub fn tabs(machines: &[(usize, &str, &Model)]) -> (Vec<Row>, Vec<Row>) {
    let (mut tabs, mut blocks) = (Vec::new(), Vec::new());
    for &(host, label, m) in machines {
        for (i, s) in m.sessions.iter().enumerate() {
            for w in &s.windows {
                let shows = |b: &crate::remote::Block| status::indicator_of(&b.newest_first());
                let tab = Row {
                    host,
                    host_label: label.to_string(),
                    session_id: s.id.clone(),
                    session: session_title(s, i),
                    window_id: w.id.clone(),
                    tab: tab_title(s, w),
                    block_id: None,
                    block: None,
                    status: status::aggregate(s.blocks.iter().filter(|b| b.window_id == w.id).map(shows)),
                };
                let o = s.overlay(&w.id);
                for b in s.blocks.iter().filter(|b| b.window_id == w.id) {
                    // a float or drawer always has a row (by its label first: `leon toggle`'s name),
                    // marked when hidden; a tile named like its tab adds nothing to the tab's row
                    let title = match o.contains(&b.id) {
                        true => {
                            let name = b.label.clone().or(b.title.clone()).unwrap_or_else(|| program_name(&b.command));
                            Some(if o.hidden(&b.id) { format!("{name} · hidden") } else { name })
                        }
                        false => b.title.clone().filter(|t| *t != tab.tab),
                    };
                    let Some(title) = title else { continue };
                    let block_id = Some(b.id.clone());
                    blocks.push(Row { block_id, block: Some(title), status: shows(b), ..tab.clone() });
                }
                tabs.push(tab);
            }
        }
    }
    (tabs, blocks)
}

/// The rows to show for `query`: with none, every tab, the ones in `recent` (window ids, most
/// recent first) first; with one, the tabs and the blocks whose title matches, best first, the
/// more recent first on a tie.
pub fn rank(tabs: &[Row], blocks: &[Row], query: &str, recent: &[String]) -> Vec<Row> {
    let used = |r: &&Row| recent.iter().position(|w| *w == r.window_id).unwrap_or(usize::MAX);
    let mut rows: Vec<&Row> = tabs.iter().collect();
    rows.sort_by_key(used); // stable: the rest keep their order
    if query.trim().is_empty() {
        return rows.into_iter().cloned().collect();
    }
    let mut titled: Vec<&Row> = blocks.iter().collect();
    titled.sort_by_key(used);
    let tab_items = rows.iter().map(|r| Item {
        name: r.text(),
        title: r.tab.clone(),
        keywords: vec![r.session.clone(), r.host_label.clone()],
        shortcut: None,
        args: None,
    });
    // a block row matches on its own title only: its tab's words would bring every block along
    let block_items = titled.iter().map(|r| Item {
        name: String::new(),
        title: r.block.clone().unwrap_or_default(),
        keywords: Vec::new(),
        shortcut: None,
        args: None,
    });
    let items: Vec<Item> = tab_items.chain(block_items).collect();
    let all: Vec<&Row> = rows.into_iter().chain(titled).collect();
    palette::filter(&items, query).into_iter().map(|i| all[i].clone()).collect()
}
