//! Serving over Tailscale (docs/architecture/tailnet.md): with `serve_tailscale` on, the server
//! also listens on this machine's Tailscale addresses, and lets a connection in only once the
//! local tailscaled says it comes from another of the owner's untagged machines (or a tag the
//! config lists). Nothing is read from a connection before that; a refusal closes it unanswered.
//! A let-in connection is served by the same code as the Unix socket, or, with `serve_web` on and
//! when it starts like HTTP, by the web page (`web.rs`).

use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use leon_hosts::{direct, localapi, shown_text};
use schemars::JsonSchema;
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::serve::{Open, Shared};
use crate::web::Site;

mod gate;
use gate::{Gate, Outcome, Turned, WINDOW};

/// How often the settings and Tailscale's status are looked at.
const POLL: Duration = Duration::from_secs(2);
/// A connection's whole decision: two status reads and a whois.
const DECIDE: Duration = Duration::from_secs(3);

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
    /// The web page's URLs while serving with `serve_web` on (the MagicDNS name first), else empty.
    pub web: Vec<String>,
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
            web: vec![],
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
    /// The token the web page holds: one per server run, made when the page is first served.
    token: OnceLock<String>,
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

/// Listening now: dropping it closes the listeners and every connection it let in.
struct Serving {
    addrs: Vec<IpAddr>,
    port: u16,
    tags: Vec<String>,
    owner: Owner,
    /// `serve_web`, and the MagicDNS name the page answers to.
    web: (bool, Option<String>),
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
        let serve_web = s.serve_web;
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
                    Ok(()) => Ok((addrs, owner, (serve_web, me.dns.clone()))),
                }
            }
        };
        let (addrs, owner, web) = match want {
            Ok(w) => w,
            Err(reason) => {
                stop(&shared, &mut serving, port, Some(reason)).await;
                continue;
            }
        };
        let same =
            |s: &Serving| s.addrs == addrs && s.port == port && s.tags == tags && s.owner == owner && s.web == web;
        if serving.as_ref().is_some_and(same) {
            continue;
        }
        stop(&shared, &mut serving, port, Some("rebinding".into())).await;
        match bind(&shared, &addrs, port, &tags, &owner, web).await {
            Ok((s, at, urls)) => {
                eprintln!("leon: tailnet: serving on {} for {}", at.join(", "), shown_text(&owner.login, 128));
                if let Some(url) = urls.first() {
                    eprintln!("leon: tailnet: the web page is at {url}");
                }
                let mut st = crate::events::lock(&shared.ctx.tailnet.status);
                (st.state, st.port, st.addresses, st.web) = ("serving".into(), port, at, urls);
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
    (st.state, st.port, st.addresses, st.reason, st.web) = (state.into(), port, vec![], reason, vec![]);
    if st.state == "off" {
        st.login = None;
    }
}

/// Listen on `port` at each of `addrs` (never another address: they are on the tailnet). One that
/// can't be bound (IPv6 turned off, say) is logged and left out; none bound is an error. Returns
/// what serves, where it listens, and the web page's URLs (none with `serve_web` off).
async fn bind(
    shared: &Arc<Shared>,
    addrs: &[IpAddr],
    port: u16,
    tags: &[String],
    owner: &Owner,
    web: (bool, Option<String>),
) -> Result<(Serving, Vec<String>, Vec<String>), String> {
    let (alive, ended) = watch::channel(());
    let (mut listeners, mut at, mut failed) = (Vec::new(), Vec::new(), None);
    for ip in addrs {
        debug_assert!(direct::on_tailnet(*ip));
        let addr = SocketAddr::new(*ip, port);
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                listeners.push(listener);
                at.push(addr.to_string());
            }
            Err(e) => {
                let why = format!("can't listen on {addr}: {e}");
                eprintln!("leon: tailnet: {why}");
                failed = Some(why);
            }
        }
    }
    let site = web.0.then(|| {
        let token = shared.ctx.tailnet.token.get_or_init(crate::web::new_token).clone();
        Arc::new(Site::new(&at, web.1.as_deref(), port, token))
    });
    let urls = site.as_ref().map(|s| s.urls.clone()).unwrap_or_default();
    let accepts = listeners
        .into_iter()
        .map(|l| tokio::spawn(accept(l, shared.clone(), tags.to_vec(), owner.clone(), site.clone(), ended.clone())))
        .collect();
    let s =
        Serving { addrs: addrs.to_vec(), port, tags: tags.to_vec(), owner: owner.clone(), web, accepts, _alive: alive };
    match (at.is_empty(), failed) {
        (true, Some(why)) => Err(why),
        _ => Ok((s, at, urls)),
    }
}

async fn accept(
    listener: TcpListener,
    shared: Arc<Shared>,
    tags: Vec<String>,
    owner: Owner,
    site: Option<Arc<Site>>,
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
        let (shared, tags, owner, site, mut ended) =
            (shared.clone(), tags.clone(), owner.clone(), site.clone(), ended.clone());
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
            tokio::select! {
                biased; // serving stopped while this was decided: not one request is served
                _ = ended.changed() => {}
                _ = crate::web::route(stream, &guard.0, site.as_deref()) => {}
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
    fn a_changed_or_tagged_account_is_not_the_owner() {
        let owner = Owner { user: 7, login: "me@example.com".into() };
        let me = localapi::Me {
            node: "n".into(),
            user: 7,
            login: "me@example.com".into(),
            ips: vec![],
            tagged: false,
            dns: None,
        };
        assert!(owner.check(&me).is_ok());
        assert!(owner.check(&localapi::Me { user: 8, ..me.clone() }).is_err());
        assert!(owner.check(&localapi::Me { login: "you@example.com".into(), ..me.clone() }).is_err());
        assert!(owner.check(&localapi::Me { tagged: true, ..me }).is_err());
    }
}
