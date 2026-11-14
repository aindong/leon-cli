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
    /// The record's effective `app` changed because an ancestor was reported or removed.
    Inherited,
    Clear,
    Evicted,
    Exited,
    Prompt,
    Reset,
}

/// What M3 forwards as `terminal.program_status_changed` / `terminal.program_status_removed`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type")]
pub enum Change {
    /// `record.app` is already resolved through the ancestors.
    #[serde(rename = "program_status_changed")]
    Changed { id: Option<String>, record: Record, reason: Reason },
    /// `ids` are sorted (root first).
    #[serde(rename = "program_status_removed")]
    Removed { ids: Vec<Option<String>>, reason: Reason },
}

#[derive(Debug, Default)]
pub struct Store {
    /// id → (last report tick, record with its own `app`, not the inherited one).
    entries: HashMap<Option<String>, (u64, Record)>,
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
        self.entries.get(&id.map(String::from)).map(|(_, r)| self.resolved(id, r))
    }

    /// Every record with `app` resolved, least recently reported first.
    pub fn records(&self) -> Vec<(Option<String>, Record)> {
        self.by_tick().into_iter().map(|(id, r)| (id.clone(), self.resolved(id.as_deref(), r))).collect()
    }

    pub fn apply(&mut self, message: Message) -> Vec<Change> {
        match message {
            Message::Query => Vec::new(),
            Message::Clear { id } => self.remove_where(Reason::Clear, |k, _| within(k.as_deref(), id.as_deref())),
            Message::Set { id, record } => self.set(id, record),
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

    /// RIS wipes every record.
    pub fn reset(&mut self) -> Vec<Change> {
        self.remove_where(Reason::Reset, |_, _| true)
    }

    fn drop_live(&mut self, reason: Reason) -> Vec<Change> {
        self.remove_where(reason, |_, r| matches!(r.state, State::Working | State::Blocked))
    }

    fn set(&mut self, id: Option<String>, record: Record) -> Vec<Change> {
        let before = self.apps();
        self.tick += 1;
        if let Some((_, old)) = self.entries.insert(id.clone(), (self.tick, record.clone())) {
            if old == record {
                return Vec::new();
            }
        }
        let mut changes = Vec::new();
        if self.entries.len() > MAX_RECORDS {
            let lru = self.entries.iter().min_by_key(|(_, (t, _))| *t).map(|(k, _)| k.clone()).expect("not empty");
            self.entries.remove(&lru);
            changes.push(Change::Removed { ids: vec![lru], reason: Reason::Evicted });
        }
        changes.push(Change::Changed {
            record: self.resolved(id.as_deref(), &record),
            id: id.clone(),
            reason: Reason::Report,
        });
        changes.extend(self.inherited(&before, Some(&id)));
        changes
    }

    fn remove_where(&mut self, reason: Reason, doomed: impl Fn(&Option<String>, &Record) -> bool) -> Vec<Change> {
        let before = self.apps();
        let mut ids: Vec<_> = self.entries.iter().filter(|(k, (_, r))| doomed(k, r)).map(|(k, _)| k.clone()).collect();
        if ids.is_empty() {
            return Vec::new();
        }
        ids.sort();
        for id in &ids {
            self.entries.remove(id);
        }
        let mut changes = vec![Change::Removed { ids, reason }];
        changes.extend(self.inherited(&before, None));
        changes
    }

    /// `changed` events for surviving records (other than `skip`) whose effective app differs from `before`.
    fn inherited(
        &self,
        before: &HashMap<Option<String>, Option<String>>,
        skip: Option<&Option<String>>,
    ) -> Vec<Change> {
        self.by_tick()
            .into_iter()
            .filter(|(id, _)| Some(*id) != skip)
            .filter(|(id, _)| before.get(*id).is_some_and(|app| *app != self.app(id.as_deref())))
            .map(|(id, r)| Change::Changed {
                id: id.clone(),
                record: self.resolved(id.as_deref(), r),
                reason: Reason::Inherited,
            })
            .collect()
    }

    fn by_tick(&self) -> Vec<(&Option<String>, &Record)> {
        let mut all: Vec<_> = self.entries.iter().collect();
        all.sort_by_key(|(_, (t, _))| *t);
        all.into_iter().map(|(id, (_, r))| (id, r)).collect()
    }

    fn apps(&self) -> HashMap<Option<String>, Option<String>> {
        self.entries.keys().map(|k| (k.clone(), self.app(k.as_deref()))).collect()
    }

    fn resolved(&self, id: Option<&str>, record: &Record) -> Record {
        Record { app: self.app(id), ..record.clone() }
    }

    /// The `app` of `id` or its nearest ancestor that has one; the root is everyone's ancestor.
    fn app(&self, mut id: Option<&str>) -> Option<String> {
        loop {
            if let Some(app) = self.entries.get(&id.map(String::from)).and_then(|(_, r)| r.app.as_ref()) {
                return Some(app.clone());
            }
            id = id?.rsplit_once('/').map(|(parent, _)| parent);
        }
    }
}

/// `key` is `root` or one of its descendants (`None` = root of everything).
fn within(key: Option<&str>, root: Option<&str>) -> bool {
    match (key, root) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some(k), Some(r)) => k.strip_prefix(r).is_some_and(|rest| rest.is_empty() || rest.starts_with('/')),
    }
}
