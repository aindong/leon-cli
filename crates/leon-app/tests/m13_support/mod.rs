//! Models as the app's client holds them, built by hand: a session per (label, tabs), each tab one
//! block with a title and status records.
#![allow(dead_code)]

use leon_app::remote::{Block, Model, Session};
use leon_core::program_status::{Kind, Record, State};
use leon_core::Window;

pub fn record(state: State) -> Record {
    Record { state, kind: None, progress: None, app: None, title: None, msg: None }
}

pub fn blocked(kind: Kind) -> Record {
    Record { kind: Some(kind), ..record(State::Blocked) }
}

pub fn working(progress: u8) -> Record {
    Record { progress: Some(progress), ..record(State::Working) }
}

/// One tab: its label (None: named by its block's title), the block's title and its records.
pub struct Tab<'a>(pub Option<&'a str>, pub &'a str, pub Vec<Record>);

/// A model of `sessions`, each (label, tabs); ids are `session:<label>`, `window:<label>/<i>`,
/// `block:<label>/<i>`.
pub fn model(sessions: Vec<(&str, Vec<Tab>)>) -> Model {
    let mut m = Model::default();
    for (label, tabs) in sessions {
        let mut s = Session {
            id: format!("session:{label}"),
            label: Some(label.into()),
            revision: 1,
            active: 0,
            windows: Vec::new(),
            blocks: Vec::new(),
            overlays: Default::default(),
            tiled_focus: Default::default(),
        };
        for (i, Tab(tab, title, status)) in tabs.into_iter().enumerate() {
            let (wid, bid) = (format!("window:{label}/{i}"), format!("block:{label}/{i}"));
            s.windows.push(Window {
                id: wid.clone(),
                label: tab.map(String::from),
                layout: None,
                focus: Some(bid.clone()),
                zoomed: None,
            });
            s.blocks.push(Block {
                id: bid,
                window_id: wid,
                label: None,
                command: vec!["/bin/zsh".into()],
                title: Some(title.into()),
                cwd: None,
                status,
            });
        }
        m.sessions.push(s);
    }
    m
}
