//! One terminal's program-status records and the change events they produce.

use std::collections::HashMap;

use serde::Serialize;

use super::{Message, Record, State};

/// Records kept per terminal (the spec requires at least 64); the least recently reported is evicted.
pub const MAX_RECORDS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Reason {
    Report,
    Clear,
    Evicted,
    Exited,
    Prompt,
    Reset,
    /// The user looked at the terminal (the app focused its pane).
    Seen,
}

/// What M3 forwards as `terminal.program_status_changed` / `terminal.program_status_removed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum Change {
    /// `record.app` is resolved through the ancestors when the event is made. A later change to an
    /// ancestor's `app` sends no event for its descendants: re-announcing them would let one tiny report
    /// fan out into 255 full records. Read `Store::get`/`records` for the current value.
    #[serde(rename = "program_status_changed")]
    Changed { id: Option<String>, record: Record, reason: Reason },
    /// `ids` are sorted (root first).
    #[serde(rename = "program_status_removed")]
    Removed { ids: Vec<Option<String>>, reason: Reason },
}

#[derive(Debug)]
struct Entry {
    /// When it was last reported, for LRU.
    tick: u64,
    /// With its own `app`, not the inherited one.
    record: Record,
}

#[derive(Debug, Default)]
pub struct Store {
    /// Keyed by id; the root record is `""`, which no valid id can be.
    entries: HashMap<String, Entry>,
    tick: u64,
}

impl Store {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The record with `app` inherited from the nearest ancestor.
    pub fn get(&self, id: Option<&str>) -> Option<Record> {
        let key = id.unwrap_or("");
        self.entries.get(key).map(|e| self.resolved(key, &e.record))
    }

    /// Every record with `app` resolved, least recently reported first.
    pub fn records(&self) -> Vec<(Option<String>, Record)> {
        self.by_tick().into_iter().map(|(k, e)| (public(k), self.resolved(k, &e.record))).collect()
    }

    /// Every record as reported (`app` not resolved), most recently reported first, uncopied.
    pub fn newest_first(&self) -> Vec<&Record> {
        self.by_tick().into_iter().rev().map(|(_, e)| &e.record).collect()
    }

    /// Applies one report.
    pub fn apply(&mut self, message: Message) -> Vec<Change> {
        match message {
            Message::Query => Vec::new(),
            Message::Clear { id } => {
                let root = id.unwrap_or_default();
                self.remove_where(Reason::Clear, |k, _| within(k, &root))
            }
            Message::Set { id, record } => self.set(id.unwrap_or_default(), record),
        }
    }

    /// The program exited: `working` and `blocked` can't be true any more.
    pub fn process_exited(&mut self) -> Vec<Change> {
        self.drop_live(Reason::Exited)
    }

    /// A new shell prompt (OSC 133 A): same rule as exit.
    pub fn prompt(&mut self) -> Vec<Change> {
        self.drop_live(Reason::Prompt)
    }

    /// The user returned to the terminal: `done` and `error` have been seen.
    pub fn acknowledge(&mut self) -> Vec<Change> {
        self.remove_where(Reason::Seen, |_, r| matches!(r.state, State::Done | State::Error))
    }

    /// RIS wipes every record.
    pub fn reset(&mut self) -> Vec<Change> {
        self.remove_where(Reason::Reset, |_, _| true)
    }

    fn drop_live(&mut self, reason: Reason) -> Vec<Change> {
        self.remove_where(reason, |_, r| matches!(r.state, State::Working | State::Blocked))
    }

    fn set(&mut self, key: String, record: Record) -> Vec<Change> {
        self.tick += 1;
        let tick = self.tick;
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.tick = tick;
            if entry.record == record {
                return Vec::new();
            }
        }
        let mut changes = Vec::new();
        if !self.entries.contains_key(&key) && self.entries.len() == MAX_RECORDS {
            let lru = self.entries.iter().min_by_key(|(_, e)| e.tick).map(|(k, _)| k.clone()).expect("full");
            self.entries.remove(&lru);
            changes.push(Change::Removed { ids: vec![public(&lru)], reason: Reason::Evicted });
        }
        let resolved = Record { app: self.app_with(&key, record.app.as_ref()), ..record.clone() };
        self.entries.insert(key.clone(), Entry { tick, record });
        changes.push(Change::Changed { id: public(&key), record: resolved, reason: Reason::Report });
        changes
    }

    fn remove_where(&mut self, reason: Reason, doomed: impl Fn(&str, &Record) -> bool) -> Vec<Change> {
        let mut keys: Vec<String> =
            self.entries.iter().filter(|(k, e)| doomed(k, &e.record)).map(|(k, _)| k.clone()).collect();
        if keys.is_empty() {
            return Vec::new();
        }
        keys.sort();
        for k in &keys {
            self.entries.remove(k);
        }
        vec![Change::Removed { ids: keys.iter().map(|k| public(k)).collect(), reason }]
    }

    fn by_tick(&self) -> Vec<(&String, &Entry)> {
        let mut all: Vec<_> = self.entries.iter().collect();
        all.sort_by_key(|(_, e)| e.tick);
        all
    }

    fn resolved(&self, key: &str, record: &Record) -> Record {
        Record { app: self.app(key), ..record.clone() }
    }

    fn app(&self, key: &str) -> Option<String> {
        self.app_with(key, self.entries.get(key).and_then(|e| e.record.app.as_ref()))
    }

    /// `own`, else the `app` of the nearest ancestor that has one; the root is everyone's ancestor.
    fn app_with(&self, key: &str, own: Option<&String>) -> Option<String> {
        let mut key = key;
        let mut found = own;
        while found.is_none() && !key.is_empty() {
            key = key.rsplit_once('/').map_or("", |(parent, _)| parent);
            found = self.entries.get(key).and_then(|e| e.record.app.as_ref());
        }
        found.cloned()
    }
}

fn public(key: &str) -> Option<String> {
    (!key.is_empty()).then(|| key.to_string())
}

/// `key` is `root` or one of its descendants (`""` = the root of everything).
fn within(key: &str, root: &str) -> bool {
    root.is_empty() || key.strip_prefix(root).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}
