//! The window's hosts as Settings › Remote Hosts changes them: each host's line, Remove, Rename
//! and Add Host…, each applied to hosts.json and to the window at once.

use super::*;
use crate::hosts::HostLine;

impl Workspace {
    /// Each host (not This Mac) as Settings › Remote Hosts lists it.
    pub fn host_lines(&self) -> Vec<HostLine> {
        let hosts = self.hosts.machines().iter().filter_map(|m| Some((m, m.host.as_ref()?)));
        hosts
            .map(|(m, h)| {
                let state = m.state();
                HostLine {
                    label: m.label.clone(),
                    endpoint: h.endpoints.join(", "),
                    connected: state == HostState::Connected,
                    note: state.note(&m.label),
                    tailnet: self.tailnet_line(m),
                }
            })
            .collect()
    }

    /// Forget the host `label`: out of hosts.json, out of the window (its server and programs
    /// keep running there). A session of it that was selected gives way to This Mac's.
    pub fn remove_host(&mut self, label: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let i = self.hosts.index_of(label).filter(|i| *i > 0).ok_or_else(|| format!("no host {label:?}"))?;
        let mut list = leon_hosts::hosts::load()?;
        list.retain(|h| h.label != label);
        leon_hosts::hosts::save(&list)?;
        if self.host == i {
            self.use_host(0);
            self.session_id = self.app.model().sessions.first().map(|s| s.id.clone());
        }
        // the hosts after it move up one
        self.host -= usize::from(self.host > i);
        let mut later = lock(&self.select_later);
        match later.as_ref().map(|(h, _)| *h) {
            Some(h) if h == i => *later = None,
            Some(h) if h > i => later.as_mut().expect("some").0 -= 1,
            _ => {}
        }
        drop(later);
        // everything else that holds a machine's index follows it
        self.attention.forget_host(i);
        self.notifier.forget_host(i);
        self.overview_forget_host(i);
        self.hosts.remove(i);
        self.settings.collapsed_hosts.retain(|l| l != label);
        self.save_settings();
        cx.notify();
        Ok(())
    }

    /// Call the host `old` `new`, in hosts.json and in the window; the label is checked as
    /// `leon hosts add` checks one.
    pub fn rename_host(&mut self, old: &str, new: &str, cx: &mut Context<Self>) -> Result<(), String> {
        let i = self.hosts.index_of(old).filter(|i| *i > 0).ok_or_else(|| format!("no host {old:?}"))?;
        if self.hosts.index_of(new).is_some_and(|j| j != i) {
            return Err(format!("{} {new:?} already", leon_hosts::hosts::TAKEN));
        }
        let mut list = leon_hosts::hosts::load()?;
        leon_hosts::hosts::rename(&mut list, old, new)?;
        leon_hosts::hosts::save(&list)?;
        self.hosts.rename(i, new);
        for l in self.settings.collapsed_hosts.iter_mut().filter(|l| *l == old) {
            *l = new.to_string();
        }
        self.save_settings();
        cx.notify();
        Ok(())
    }

    /// Add Host… from Settings: bring this window forward with the prompt open.
    pub fn add_host(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.activate_window();
        self.ask_host(None, cx);
    }
}
