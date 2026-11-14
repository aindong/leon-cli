//! A terminal's OSC 7501 program status as its users see it: the records, the user arriving at
//! the pane, and a report made for the terminal from outside (`leon notify`).

use super::*;

impl Terminal {
    /// The program-status records (OSC 7501), least recently reported first.
    pub fn program_status(&self) -> Vec<(Option<String>, Record)> {
        lock(&self.tracker).store.records()
    }

    /// Look at the program-status records, most recently reported first, without copying them
    /// (the app reads them on every poll).
    pub fn with_program_status<R>(&self, f: impl FnOnce(&[&Record]) -> R) -> R {
        f(&lock(&self.tracker).store.newest_first())
    }

    /// The user returned to this terminal: drop its `done` and `error` records (spec lifetime) and
    /// report the removal like any other. Don't hold the server's state lock: the notice takes it.
    pub fn acknowledge(&self) {
        let gone = lock(&self.tracker).store.acknowledge();
        if !gone.is_empty() {
            self.generation.send_modify(|g| *g += 1);
        }
        gone.into_iter().for_each(|c| (self.notify)(Notice::ProgramStatus(c)));
    }

    /// Set this terminal's root record as an OSC 7501 report from its program would (`leon notify`):
    /// built into a report and run through the same parser, so the spec's limits and text rules
    /// hold. A title or msg the parser would drop is refused instead, so the caller hears of it.
    pub fn report_status(&self, state: &str, title: Option<&str>, msg: Option<&str>) -> Result<(), String> {
        use base64::Engine as _;
        let b64 = |t: &str| base64::engine::general_purpose::STANDARD.encode(t);
        let mut payload = format!("state={state}");
        for (key, text) in [("title", title), ("msg", msg)] {
            if let Some(t) = text {
                payload.push_str(&format!(":{key}={}", b64(t)));
            }
        }
        use crate::program_status::Ignored;
        let message = crate::program_status::parse(payload.as_bytes()).map_err(|e| match e {
            Ignored::TooLong(key) => format!("the {key} is too long"),
            _ => format!("{state:?} is not a state: working, blocked, done, error, idle or clear"),
        })?;
        if let crate::program_status::Message::Set { record, .. } = &message {
            if title.is_some() && record.title.is_none() || msg.is_some() && record.msg.is_none() {
                return Err("a title or body must be one line of visible text".into());
            }
        }
        let changes = lock(&self.tracker).store.apply(message);
        if !changes.is_empty() {
            self.generation.send_modify(|g| *g += 1);
        }
        changes.into_iter().for_each(|c| (self.notify)(Notice::ProgramStatus(c)));
        Ok(())
    }
}
