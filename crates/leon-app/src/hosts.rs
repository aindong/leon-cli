//! The machines the window shows (docs/architecture.md, "The app's hosts"): This Mac, then each
//! host from the hosts list, reached through a `leon_hosts::Link` whose ssh never prompts. A thread
//! per connection attempt does the slow part; the UI thread picks the result up in `poll`.

use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use leon_hosts::tailscale::{self, Tailnet};
use leon_hosts::{Host, Link};

use crate::remote::{LinkState, Remote};

pub const THIS_MAC: &str = "This Mac";
/// The waits between fresh connection attempts to a host that failed.
const BACKOFF: (Duration, Duration) = (Duration::from_secs(2), Duration::from_secs(60));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HostState {
    Connecting,
    Connected,
    /// The link dropped and is coming back: which try, and the wait before the next.
    Reconnecting {
        attempt: u32,
        retry_in: Duration,
    },
    /// ssh's reason, or the link's.
    Unreachable(String),
    /// ssh wanted a password, a passphrase or a host key answer, which only a terminal can give.
    NeedsLogin(String),
}

impl HostState {
    /// What the sidebar says under the host's label; nothing once connected.
    pub fn note(&self, label: &str) -> Option<String> {
        match self {
            HostState::Connecting => Some("connecting…".into()),
            HostState::Connected => None,
            HostState::Reconnecting { attempt, retry_in } => {
                Some(format!("reconnecting (try {attempt}, next in {:.1} s)", retry_in.as_secs_f32()))
            }
            HostState::Unreachable(why) => Some(format!("unreachable: {why}")),
            HostState::NeedsLogin(_) => Some(format!("needs login: run `leon hosts check {label}` in a terminal")),
        }
    }
}

/// Sort a failure: in BatchMode ssh fails where it would have asked something, and those need a
/// terminal; anything else is a host that can't be reached.
pub fn classify(why: String) -> HostState {
    let why = leon_hosts::shown_text(&why, 200); // the host's words go on screen
    let l = why.to_lowercase();
    // the last: Tailscale SSH's browser check ("To authenticate, visit: …")
    let asks = [
        "permission denied",
        "host key verification failed",
        "passphrase",
        "password",
        "authentication",
        "to authenticate",
    ];
    match asks.iter().any(|w| l.contains(w)) {
        true => HostState::NeedsLogin(why),
        false => HostState::Unreachable(why),
    }
}

/// What a connection attempt's thread hands over.
#[derive(Default)]
struct Attempt {
    state: Option<HostState>,
    done: Option<(Remote, Link)>,
    running: bool,
}

pub struct Machine {
    pub label: String,
    /// None for This Mac.
    pub host: Option<Host>,
    remote: Option<Rc<Remote>>,
    /// Kept while its remote is in use (the remote's socket is in the link's dir).
    _link: Option<Link>,
    attempt: Arc<Mutex<Attempt>>,
    /// The failure shown until the next attempt works, and when that attempt is due.
    failed: Option<HostState>,
    retry_at: Option<Instant>,
    backoff: Duration,
}

impl Machine {
    fn new(label: String, host: Option<Host>, remote: Option<Rc<Remote>>) -> Machine {
        Machine {
            label,
            host,
            remote,
            _link: None,
            attempt: Arc::default(),
            failed: None,
            retry_at: None,
            backoff: BACKOFF.0,
        }
    }

    pub fn remote(&self) -> Option<&Rc<Remote>> {
        self.remote.as_ref()
    }

    pub fn is_local(&self) -> bool {
        self.host.is_none()
    }

    pub fn state(&self) -> HostState {
        match &self.remote {
            Some(r) => match r.link_state() {
                LinkState::Up => HostState::Connected,
                LinkState::Reconnecting { attempt, retry_in } => HostState::Reconnecting { attempt, retry_in },
            },
            None => self.failed.clone().unwrap_or(HostState::Connecting),
        }
    }
}

pub struct Hosts {
    machines: Vec<Machine>,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// The last tailnet status read (None until one comes), kept while the next is read.
    tailnet: Arc<Mutex<Option<Result<Tailnet, String>>>>,
}

impl Hosts {
    /// This Mac on `local`, then `hosts`, each starting to connect now. `wake` runs (on any
    /// thread) whenever a host's state or model changes.
    pub fn new(local: Rc<Remote>, hosts: Vec<Host>, wake: Arc<dyn Fn() + Send + Sync>) -> Hosts {
        let this_mac = Machine::new(THIS_MAC.into(), None, Some(local));
        let any = !hosts.is_empty();
        let mut out = Hosts { machines: vec![this_mac], wake, tailnet: Arc::default() };
        hosts.into_iter().for_each(|h| out.add(h));
        if any {
            out.read_tailnet(); // which hosts are tailnet machines
        }
        out
    }

    /// Read the tailnet's status again on a thread of its own (it may take up to its deadline);
    /// `tailnet` has it once it comes. False when there is no Tailscale client here.
    pub fn read_tailnet(&self) -> bool {
        if tailscale::client().is_none() {
            return false;
        }
        let (into, wake) = (self.tailnet.clone(), self.wake.clone());
        let read = move || {
            let status = tailscale::status();
            *into.lock().unwrap_or_else(PoisonError::into_inner) = Some(status);
            wake();
        };
        let _ = std::thread::Builder::new().name("leon-tailnet".into()).spawn(read);
        true
    }

    /// The last tailnet status read, if one has come.
    pub fn tailnet(&self) -> Option<Result<Tailnet, String>> {
        self.tailnet.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Show `host` and start connecting to it.
    pub fn add(&mut self, host: Host) {
        let mut m = Machine::new(host.label.clone(), Some(host), None);
        Self::spawn(&mut m, self.wake.clone());
        self.machines.push(m);
    }

    pub fn machines(&self) -> &[Machine] {
        &self.machines
    }

    pub fn local(&self) -> &Rc<Remote> {
        self.machines[0].remote.as_ref().expect("This Mac always has its remote")
    }

    pub fn index_of(&self, label: &str) -> Option<usize> {
        self.machines.iter().position(|m| m.label == label)
    }

    /// The remotes of the hosts that are connected (or reconnecting), with their index.
    pub fn remotes(&self) -> impl Iterator<Item = (usize, &Rc<Remote>)> {
        self.machines.iter().enumerate().filter_map(|(i, m)| m.remote.as_ref().map(|r| (i, r)))
    }

    /// Pick up finished attempts, drop links that gave up, start due retries. True when a host
    /// gained or lost its remote.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let wake = self.wake.clone();
        for m in self.machines.iter_mut().filter(|m| !m.is_local()) {
            if m.remote.as_ref().is_some_and(|r| !r.alive()) {
                let why = m._link.as_ref().and_then(Link::last_error);
                (m.remote, m._link) = (None, None);
                m.failed = Some(classify(why.unwrap_or_else(|| "the link dropped".into())));
                m.retry_at = Some(Instant::now() + m.backoff);
                changed = true;
            }
            let mut a = m.attempt.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some((remote, link)) = a.done.take() {
                let w = wake.clone();
                remote.on_change(move || w());
                (m.remote, m._link, m.failed, m.backoff) = (Some(Rc::new(remote)), Some(link), None, BACKOFF.0);
                changed = true;
            } else if let Some(st) = a.state.take() {
                m.failed = Some(st);
                m.retry_at = Some(Instant::now() + m.backoff);
                m.backoff = (m.backoff * 2).min(BACKOFF.1);
            }
            let due = m.remote.is_none() && !a.running && m.retry_at.is_some_and(|t| Instant::now() >= t);
            drop(a);
            if due {
                m.retry_at = None;
                Self::spawn(m, wake.clone());
            }
        }
        changed
    }

    /// One attempt on a thread of its own: open a link (ssh in BatchMode) and attach through it.
    fn spawn(m: &mut Machine, wake: Arc<dyn Fn() + Send + Sync>) {
        let Some(host) = m.host.clone() else { return };
        let attempt = m.attempt.clone();
        attempt.lock().unwrap_or_else(PoisonError::into_inner).running = true;
        let run = move || {
            let result = connect(&host);
            let mut a = attempt.lock().unwrap_or_else(PoisonError::into_inner);
            match result {
                Ok(done) => a.done = Some(done),
                Err(st) => a.state = Some(st),
            }
            a.running = false;
            drop(a);
            wake();
        };
        if std::thread::Builder::new().name("leon-host-connect".into()).spawn(run).is_err() {
            m.attempt.lock().unwrap_or_else(PoisonError::into_inner).running = false;
        }
    }
}

fn connect(host: &Host) -> Result<(Remote, Link), HostState> {
    let endpoint = host.endpoint().map_err(HostState::Unreachable)?;
    let link = Link::open(&host.label, &endpoint, true).map_err(|e| HostState::Unreachable(e.to_string()))?;
    link.never_prompt(); // no terminal to answer in: ssh fails instead, and the row says so
    match Remote::connect_retrying(link.socket(), leon_server::resume::link_hold()) {
        Ok(r) => Ok((r, link)),
        Err(e) => Err(classify(link.last_error().unwrap_or(e))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts_ssh_refused_need_a_login() {
        for why in [
            "me@box: Permission denied (publickey,password).",
            "Host key verification failed.",
            "Enter passphrase for key: ",
            "# Tailscale SSH requires an additional check. To authenticate, visit: https://login.example/a/1",
        ] {
            assert!(matches!(classify(why.into()), HostState::NeedsLogin(_)), "{why}");
        }
        let why = "ssh: connect to host box port 22: Connection refused";
        assert_eq!(classify(why.into()), HostState::Unreachable(why.into()));
        assert_eq!(
            classify("x: Permission denied".into()).note("box").unwrap(),
            "needs login: run `leon hosts check box` in a terminal"
        );
        assert_eq!(HostState::Connected.note("box"), None);
        let hostile = format!("\u{1b}]0;x\u{7}\u{202e}{}", "A".repeat(100_000));
        let HostState::Unreachable(shown) = classify(hostile) else { panic!() };
        assert!(
            shown.chars().count() <= 200 && !shown.contains(['\u{1b}', '\u{202e}']),
            "a host's error is shown tamed"
        );
    }
}
