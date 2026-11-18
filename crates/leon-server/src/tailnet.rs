//! Serving over Tailscale (docs/architecture/tailnet.md): with `serve_tailscale` on, the server
//! also listens on this machine's Tailscale addresses, and lets a connection in only once the
//! local tailscaled says it comes from another of the owner's untagged machines (or a tag the
//! config lists). Nothing is read from a connection before that; a refusal closes it unanswered.
//! A let-in connection is served by the same code as the Unix socket.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use leon_hosts::{direct, localapi, shown_text};
use schemars::JsonSchema;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::serve::{Open, Shared};

/// How often the settings and Tailscale's status are looked at.
const POLL: Duration = Duration::from_secs(2);
/// A connection's whole decision: two status reads and a whois.
const DECIDE: Duration = Duration::from_secs(3);
/// Refusals an address may have in `WINDOW` before its connections are closed unasked.
const FAILS: u32 = 5;
const WINDOW: Duration = Duration::from_secs(60);
/// Addresses the refusal count remembers; when full, a new address is refused unasked.
const MOST_ADDRS: usize = 4096;
/// Connections waiting for a decision, in all and from one address.
const PENDING: usize = 32;
const PENDING_PER: usize = 4;

/// What `server.status` says about serving over Tailscale.
#[derive(Serialize, JsonSchema, Clone, Debug)]
pub struct TailnetStatus {
    /// `off` (serve_tailscale is false), `serving`, or `stopped` (on, but not serving: `reason` says why).
    pub state: String,
    /// The port it serves on (serve_tailscale_port).
    pub port: u16,
    /// Where it listens, as `ADDRESS:PORT`: this machine's Tailscale addresses only.
    pub addresses: Vec<String>,
    /// The Tailscale login it lets in: the one this machine was signed in as when serving started.
    pub login: Option<String>,
    /// Why it isn't serving.
    pub reason: Option<String>,
    /// The last connection turned away.
    pub last_refusal: Option<Refusal>,
}

#[derive(Serialize, JsonSchema, Clone, Debug)]
pub struct Refusal {
    /// The caller's `ADDRESS:PORT`.
    pub peer: String,
    pub reason: String,
    /// When, in seconds since the Unix epoch.
    pub at: u64,
}

impl Default for TailnetStatus {
    fn default() -> Self {
        TailnetStatus {
            state: "off".into(),
            port: direct::PORT,
            addresses: vec![],
            login: None,
            reason: None,
            last_refusal: None,
        }
    }
}

/// The server's serving state, shared by the supervisor, the accept loops and `server.status`.
#[derive(Default)]
pub struct Tailnet {
    status: Mutex<TailnetStatus>,
    gate: Mutex<Gate>,
    /// When a caller from outside the tailnet ranges, and one turned away for too many waiting,
    /// were last logged: each once a `WINDOW` at most.
    strays: Mutex<Option<Instant>>,
    busy: Mutex<Option<Instant>>,
}

impl Tailnet {
    pub fn status(&self) -> TailnetStatus {
        crate::events::lock(&self.status).clone()
    }

    /// `refuse`, unless one like it was logged less than a `WINDOW` ago (`last`).
    fn refuse_now_and_then(&self, last: &Mutex<Option<Instant>>, peer: SocketAddr, reason: &str) {
        let mut at = crate::events::lock(last);
        if at.is_none_or(|t| t.elapsed() >= WINDOW) {
            *at = Some(Instant::now());
            drop(at);
            self.refuse(peer, &format!("{reason} (more such are closed unlogged for a minute)"));
        }
    }

    /// One line in the server log, and the status's `last_refusal`.
    fn refuse(&self, peer: SocketAddr, reason: &str) {
        let reason = shown_text(reason, 300);
        eprintln!("leon: tailnet: refused {peer}: {reason}");
        let at = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        crate::events::lock(&self.status).last_refusal = Some(Refusal { peer: peer.to_string(), reason, at });
    }
}

/// What a decision means for an address's refusal count.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Outcome {
    /// Let in: the count is cleared.
    In,
    /// The caller was refused: it counts.
    Refused,
    /// tailscaled failed, not the caller: the count stays as it is.
    Unsure,
}

/// The account serving started for: a changed one stops serving.
#[derive(Clone, PartialEq, Debug)]
struct Owner {
    user: u64,
    login: String,
}

impl Owner {
    fn check(&self, me: &localapi::Me) -> Result<(), String> {
        match (me.user, &me.login) == (self.user, &self.login) && !me.tagged {
            true => Ok(()),
            false => Err(format!(
                "Tailscale is now signed in as {}, not {}: turn serve_tailscale off and on to serve as {0}",
                shown_text(&me.login, 128),
                shown_text(&self.login, 128)
            )),
        }
    }
}

/// Refusal counts and connections waiting, by address.
#[derive(Default)]
struct Gate {
    /// Refusals since a time, and whether the limit was logged.
    fails: HashMap<IpAddr, (u32, Instant, bool)>,
    pending: HashMap<IpAddr, usize>,
}

enum Turned {
    /// Too many refusals lately; `first` when this is the first connection turned for it.
    Limited {
        first: bool,
    },
    Busy,
}

impl Gate {
    fn enter(&mut self, ip: IpAddr) -> Result<(), Turned> {
        if self.fails.len() >= MOST_ADDRS {
            self.fails.retain(|_, f| f.1.elapsed() < WINDOW);
        }
        let full = self.fails.len() >= MOST_ADDRS;
        match self.fails.get_mut(&ip) {
            Some(f) if f.1.elapsed() >= WINDOW => {
                self.fails.remove(&ip);
            }
            Some(f) if f.0 >= FAILS => return Err(Turned::Limited { first: !std::mem::replace(&mut f.2, true) }),
            Some(_) => {}
            None if full => return Err(Turned::Limited { first: false }),
            None => {}
        }
        let mine = self.pending.get(&ip).copied().unwrap_or(0);
        if mine >= PENDING_PER || self.pending.values().sum::<usize>() >= PENDING {
            return Err(Turned::Busy);
        }
        self.pending.insert(ip, mine + 1);
        Ok(())
    }

    /// The decision is made: a refusal counts against the address, a let-in clears its count.
    fn leave(&mut self, ip: IpAddr, outcome: Outcome) {
        match self.pending.get_mut(&ip) {
            Some(n) if *n > 1 => *n -= 1,
            _ => {
                self.pending.remove(&ip);
            }
        }
        match outcome {
            Outcome::Refused => self.fails.entry(ip).or_insert((0, Instant::now(), false)).0 += 1,
            Outcome::In => {
                self.fails.remove(&ip);
            }
            Outcome::Unsure => {}
        }
    }
}

/// Listening now: dropping it closes the listeners and every connection it let in.
struct Serving {
    addrs: Vec<IpAddr>,
    port: u16,
    tags: Vec<String>,
    owner: Owner,
    accepts: Vec<tokio::task::JoinHandle<()>>,
    /// Let-in connections end when this drops.
    _alive: watch::Sender<()>,
}

impl Drop for Serving {
    /// A dropped `JoinHandle` only detaches its task: abort the accept loops, so their listeners
    /// close however this goes away (`stop`, or the server aborting the supervisor).
    fn drop(&mut self) {
        self.accepts.iter().for_each(|a| a.abort());
    }
}

/// Follow the settings and Tailscale's status for as long as the server runs: bind, rebind or
/// stop as they change. The server aborts it on stop, which drops what is serving.
pub(crate) async fn supervise(shared: Arc<Shared>) {
    let mut serving: Option<Serving> = None;
    // the account to serve, pinned at the first good status after a serve_tailscale* setting changed
    let mut pin: Option<(u64, Owner)> = None;
    // a status call still out: a hung tailscaled gets one thread, not one per tick
    let mut job: Option<tokio::task::JoinHandle<Result<localapi::Me, String>>> = None;
    let mut tick = tokio::time::interval(POLL);
    loop {
        tick.tick().await;
        let (s, generation) = shared.ctx.settings().await;
        let (on, port, tags) = (s.serve_tailscale, s.serve_tailscale_port, s.serve_tailscale_tags);
        if !on {
            stop(&shared, &mut serving, port, None).await;
            pin = None;
            continue;
        }
        let polled =
            tokio::time::timeout(DECIDE, job.get_or_insert_with(|| tokio::task::spawn_blocking(localapi::status)))
                .await;
        let status = match polled {
            Ok(done) => {
                job = None;
                done.unwrap_or_else(|_| Err("the LocalAPI call failed".into()))
            }
            Err(_) => Err("tailscaled didn't answer in time".into()), // the same call is awaited next tick
        };
        let want = match status {
            Err(e) => Err(e),
            Ok(me) if me.tagged => Err("this machine is tagged on the tailnet, so it has no owner to serve".into()),
            Ok(me) => {
                let owner = match &pin {
                    Some((g, o)) if *g == generation => o.clone(),
                    _ => pin.insert((generation, Owner { user: me.user, login: me.login.clone() })).1.clone(),
                };
                let addrs: Vec<IpAddr> = me.ips.iter().copied().filter(|ip| direct::on_tailnet(*ip)).collect();
                match owner.check(&me) {
                    Err(e) => Err(e),
                    Ok(()) if addrs.is_empty() => Err("this machine has no Tailscale address to serve on".into()),
                    Ok(()) => Ok((addrs, owner)),
                }
            }
        };
        let (addrs, owner) = match want {
            Ok(w) => w,
            Err(reason) => {
                stop(&shared, &mut serving, port, Some(reason)).await;
                continue;
            }
        };
        if serving.as_ref().is_some_and(|s| s.addrs == addrs && s.port == port && s.tags == tags && s.owner == owner) {
            continue;
        }
        stop(&shared, &mut serving, port, Some("rebinding".into())).await;
        match bind(&shared, &addrs, port, &tags, &owner).await {
            Ok((s, at)) => {
                eprintln!("leon: tailnet: serving on {} for {}", at.join(", "), shown_text(&owner.login, 128));
                let mut st = crate::events::lock(&shared.ctx.tailnet.status);
                (st.state, st.port, st.addresses) = ("serving".into(), port, at);
                (st.login, st.reason) = (Some(owner.login.clone()), None);
                serving = Some(s);
            }
            Err(reason) => stop(&shared, &mut serving, port, Some(reason)).await,
        }
    }
}

/// Close the listeners and let-in connections, and say why (`None`: the setting is off). A new
/// state or reason goes to the log once.
async fn stop(shared: &Shared, serving: &mut Option<Serving>, port: u16, reason: Option<String>) {
    if let Some(mut s) = serving.take() {
        let accepts = std::mem::take(&mut s.accepts);
        drop(s); // ends the let-in connections
        for a in accepts {
            a.abort();
            let _ = a.await; // the listener is closed once its task is gone
        }
    }
    if reason.as_deref() == Some("rebinding") {
        return;
    }
    let state = if reason.is_some() { "stopped" } else { "off" };
    let mut st = crate::events::lock(&shared.ctx.tailnet.status);
    if st.state != state || st.reason != reason {
        match &reason {
            Some(r) => eprintln!("leon: tailnet: stopped: {}", shown_text(r, 300)),
            None if st.state != "off" => eprintln!("leon: tailnet: off"),
            None => {}
        }
    }
    (st.state, st.port, st.addresses, st.reason) = (state.into(), port, vec![], reason);
    if st.state == "off" {
        st.login = None;
    }
}

/// Listen on `port` at each of `addrs` (never another address: they are on the tailnet). One that
/// can't be bound (IPv6 turned off, say) is logged and left out; none bound is an error. Returns
/// what serves and where it listens.
async fn bind(
    shared: &Arc<Shared>,
    addrs: &[IpAddr],
    port: u16,
    tags: &[String],
    owner: &Owner,
) -> Result<(Serving, Vec<String>), String> {
    let (alive, ended) = watch::channel(());
    let (mut accepts, mut at, mut failed) = (Vec::new(), Vec::new(), None);
    for ip in addrs {
        debug_assert!(direct::on_tailnet(*ip));
        let addr = SocketAddr::new(*ip, port);
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                let (shared, tags, owner, ended) = (shared.clone(), tags.to_vec(), owner.clone(), ended.clone());
                accepts.push(tokio::spawn(accept(listener, shared, tags, owner, ended)));
                at.push(addr.to_string());
            }
            Err(e) => {
                let why = format!("can't listen on {addr}: {e}");
                eprintln!("leon: tailnet: {why}");
                failed = Some(why);
            }
        }
    }
    let s = Serving { addrs: addrs.to_vec(), port, tags: tags.to_vec(), owner: owner.clone(), accepts, _alive: alive };
    match (at.is_empty(), failed) {
        (true, Some(why)) => Err(why),
        _ => Ok((s, at)),
    }
}

async fn accept(
    listener: TcpListener,
    shared: Arc<Shared>,
    tags: Vec<String>,
    owner: Owner,
    ended: watch::Receiver<()>,
) {
    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(a) => a,
            Err(_) => {
                tokio::time::sleep(Duration::from_millis(50)).await; // e.g. out of fds: don't spin
                continue;
            }
        };
        let tn = &shared.ctx.tailnet;
        if !direct::on_tailnet(peer.ip()) {
            // no whois and no count to fill: a neighbour routing here costs nothing but a line
            tn.refuse_now_and_then(&tn.strays, peer, "not a machine's Tailscale address");
            continue; // dropping the stream closes it
        }
        match crate::events::lock(&tn.gate).enter(peer.ip()) {
            Ok(()) => {}
            Err(Turned::Limited { first }) => {
                if first {
                    eprintln!(
                        "leon: tailnet: too many refused connections from {}: closing its connections for a minute",
                        peer.ip()
                    );
                }
                continue;
            }
            Err(Turned::Busy) => {
                tn.refuse_now_and_then(&tn.busy, peer, "too many connections waiting to be let in");
                continue;
            }
        }
        let (shared, tags, owner, mut ended) = (shared.clone(), tags.clone(), owner.clone(), ended.clone());
        tokio::spawn(async move {
            let mut job = tokio::task::spawn_blocking(move || decide(peer, &owner, &tags));
            let decided = tokio::time::timeout(DECIDE, &mut job).await;
            let late = decided.is_err();
            let decided = match decided {
                Ok(Ok(d)) => d,
                Ok(Err(_)) => Err(("the LocalAPI call failed".into(), Outcome::Unsure)),
                Err(_) => Err(("tailscaled didn't answer in time".into(), Outcome::Unsure)),
            };
            let tn = &shared.ctx.tailnet;
            if let Err((reason, outcome)) = decided {
                tn.refuse(peer, &reason);
                drop(stream); // closed unread, at once
                if late {
                    let _ = job.await; // the slot is free only once its thread is: PENDING caps those too
                }
                return crate::events::lock(&tn.gate).leave(peer.ip(), outcome);
            }
            crate::events::lock(&tn.gate).leave(peer.ip(), Outcome::In);
            shared.open.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            shared.touch();
            let guard = Open(shared.clone());
            direct::keepalive(&stream); // a caller gone from the tailnet is noticed, as on its side
            let (read, write) = stream.into_split();
            tokio::select! {
                biased; // serving stopped while this was decided: not one request is served
                _ = ended.changed() => {}
                _ = crate::serve::connection(read, write, &guard.0, crate::agents::Peer::Remote) => {}
            }
        });
    }
}

/// Whether `peer` comes in: this machine is still signed in as `owner`, before and after whois,
/// and whois says the peer is one of the owner's other machines. The error's outcome says whether
/// it was the peer that failed (it counts toward the rate limit) or tailscaled.
fn decide(peer: SocketAddr, owner: &Owner, tags: &[String]) -> Result<(), (String, Outcome)> {
    let ours = |e: String| (e, Outcome::Unsure);
    let me = localapi::status().map_err(ours)?;
    owner.check(&me).map_err(ours)?;
    let who = localapi::whois(peer).map_err(ours)?;
    let again = localapi::status().map_err(ours)?;
    owner.check(&again).map_err(ours)?;
    if again.node != me.node {
        return Err(ours("Tailscale changed while deciding".into()));
    }
    localapi::admit(&me, &who, tags).map_err(|e| (e, Outcome::Refused))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_limits_refusals_and_waiting() {
        let mut g = Gate::default();
        let ip: IpAddr = "100.64.0.9".parse().unwrap();
        for _ in 0..FAILS {
            assert!(g.enter(ip).is_ok());
            g.leave(ip, Outcome::Refused);
        }
        assert!(matches!(g.enter(ip), Err(Turned::Limited { first: true })));
        assert!(matches!(g.enter(ip), Err(Turned::Limited { first: false })), "logged once");
        let other: IpAddr = "100.64.0.10".parse().unwrap();
        for _ in 0..PENDING_PER {
            assert!(g.enter(other).is_ok());
        }
        assert!(matches!(g.enter(other), Err(Turned::Busy)), "per address");
        g.leave(other, Outcome::Refused);
        g.leave(other, Outcome::Unsure);
        assert_eq!(g.fails[&other].0, 1, "tailscaled failing doesn't clear or add to a count");
        assert!(g.enter(other).is_ok());
        g.leave(other, Outcome::In);
        assert!(!g.fails.contains_key(&other), "a let-in clears its count");
        let mut full = Gate::default();
        for i in 0..MOST_ADDRS as u32 {
            full.fails.insert(IpAddr::from((0x6440_0000 + i).to_be_bytes()), (1, Instant::now(), false));
        }
        assert!(matches!(full.enter("100.127.0.1".parse().unwrap()), Err(Turned::Limited { .. })), "full: refused");
    }

    #[test]
    fn a_changed_or_tagged_account_is_not_the_owner() {
        let owner = Owner { user: 7, login: "me@example.com".into() };
        let me = localapi::Me { node: "n".into(), user: 7, login: "me@example.com".into(), ips: vec![], tagged: false };
        assert!(owner.check(&me).is_ok());
        assert!(owner.check(&localapi::Me { user: 8, ..me.clone() }).is_err());
        assert!(owner.check(&localapi::Me { login: "you@example.com".into(), ..me.clone() }).is_err());
        assert!(owner.check(&localapi::Me { tagged: true, ..me }).is_err());
    }
}
