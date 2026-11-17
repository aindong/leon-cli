//! `-H HOST` and `leon hosts` (docs/architecture.md, "Remote hosts").

use leon_core::ApiError;
use leon_hosts::{hosts, tailscale, Endpoint, Host, Link};
use serde_json::{json, Value};

use crate::client::{Fail, Server};
use crate::commands::print_json;
use crate::{Cli, HostsCmd};

/// The server a command talks to: this machine's, or `-H`'s through a link. `autostart` says
/// whether the host may start a server when none runs.
pub fn server(cli: &Cli, autostart: bool) -> Result<Server, Fail> {
    let Some(name) = &cli.host else { return Ok(Server::local()) };
    let (name, endpoint) = resolve(name)?;
    // LEON_SESSION and LEON_BLOCK name objects of this machine's server, not the host's. Done
    // before the link's threads start.
    std::env::remove_var("LEON_SESSION");
    std::env::remove_var("LEON_BLOCK");
    let link = Link::open(&name, &endpoint, autostart).map_err(|e| Fail::Other(format!("can't open a link: {e}")))?;
    Ok(Server::over(&name, link))
}

/// A `-H` value: an endpoint (named by its host in messages) or a label from the hosts list.
fn resolve(name: &str) -> Result<(String, Endpoint), Fail> {
    if name.contains("://") {
        let e: Endpoint = name.parse().map_err(Fail::Usage)?;
        return Ok((e.host.clone(), e));
    }
    hosts::check_label(name)
        .map_err(|_| Fail::Usage(format!("-H {name:?}: not a host label or ssh:// or tailnet:// endpoint")))?;
    let list = hosts::load().map_err(Fail::Other)?;
    let host = list.iter().find(|h| h.label == name).ok_or_else(|| unknown(name))?;
    Ok((host.label.clone(), host.endpoint().map_err(Fail::Usage)?))
}

/// Every label in `labels` is in `list`.
fn known(list: &[Host], labels: &[String]) -> Result<(), Fail> {
    match labels.iter().find(|l| !list.iter().any(|h| h.label == **l)) {
        Some(missing) => Err(unknown(missing)),
        None => Ok(()),
    }
}

fn unknown(label: &str) -> Fail {
    Fail::Api(ApiError::not_found(format!("no host {label:?}; add it with `leon hosts add {label} ssh://…`")))
}

pub fn run(cli: &Cli, cmd: &HostsCmd) -> Result<(), Fail> {
    let mut list = hosts::load().map_err(Fail::Other)?;
    match cmd {
        HostsCmd::List => {
            // which hosts are tailnet machines; without Tailscale the list is as it always was
            let tailnet = if list.is_empty() { None } else { tailscale::status().ok() };
            let on = |h: &Host| tailnet.as_ref().and_then(|t| t.machine_for(h).map(|m| (t, m)));
            if cli.json {
                let rows = list.iter().map(|h| {
                    let mut v = serde_json::to_value(h).expect("json");
                    if let Some((t, m)) = on(h) {
                        v["tailnet"] = json!({ "login": t.login, "online": m.online, "ssh": m.ssh });
                    }
                    v
                });
                print_json(&Value::Array(rows.collect()));
            }
            for h in list.iter().filter(|_| !cli.json) {
                let note = on(h).map(|(t, m)| {
                    let mut n = vec![t.identity()];
                    n.extend((!m.online).then(|| "offline".to_string()));
                    n.extend((!m.ssh).then(|| "no Tailscale SSH".to_string()));
                    format!("  ({})", n.join(", "))
                });
                println!("{}  {}{}", h.label, h.endpoints.join(" "), note.unwrap_or_default());
            }
        }
        HostsCmd::Add { label, icon, tailscale: true, .. } => return add_tailnet(&mut list, label.as_deref(), icon),
        HostsCmd::Add { label, endpoints, icon, .. } => {
            let label = label.clone().expect("clap requires it without --tailscale");
            let host = Host { label, endpoints: endpoints.clone(), icon: icon.clone() };
            hosts::add(&mut list, host).map_err(added)?;
            hosts::save(&list).map_err(Fail::Other)?;
        }
        HostsCmd::Remove { labels } => {
            known(&list, labels)?;
            list.retain(|h| !labels.contains(&h.label));
            hosts::save(&list).map_err(Fail::Other)?;
        }
        HostsCmd::Check { labels } => return check(cli, &list, labels),
    }
    Ok(())
}

/// Why `hosts::add` refused: a label in use is not a bad command line.
fn added(e: String) -> Fail {
    match e.starts_with(hosts::TAKEN) {
        true => Fail::Other(e),
        false => Fail::Usage(e),
    }
}

/// `leon hosts add --tailscale [MACHINE]`: the machine named, or the one picked from the list on
/// stdin, added as a plain host.
fn add_tailnet(list: &mut Vec<Host>, name: Option<&str>, icon: &Option<String>) -> Result<(), Fail> {
    let tailnet = tailscale::status().map_err(Fail::Other)?;
    let machine = match name {
        Some(n) => tailnet.find(n).map_err(|e| Fail::Api(ApiError::not_found(e)))?,
        None => pick(&tailnet)?,
    };
    let host =
        tailnet.entry(machine, icon.clone()).map_err(|e| Fail::Other(format!("can't add {}: {e}", machine.shown())))?;
    let label = host.label.clone();
    hosts::add(list, host.clone()).map_err(added)?;
    hosts::save(list).map_err(Fail::Other)?;
    println!("added {label}: {}", host.endpoints[0]);
    if !machine.ssh {
        println!("{}", tailscale::ssh_advice(&label));
    }
    println!("next, once: leon hosts check {label} (accepts its host key, or passes Tailscale's sign-in check)");
    Ok(())
}

/// The tailnet's machines, numbered, and the one whose number or name is typed.
fn pick(tailnet: &tailscale::Tailnet) -> Result<&tailscale::Machine, Fail> {
    if tailnet.machines.is_empty() {
        return Err(Fail::Other("no other machines on your tailnet".into()));
    }
    println!("Machines on your tailnet ({}):", tailnet.identity());
    let width = tailnet.machines.iter().map(|m| m.shown().chars().count()).max().unwrap_or(0);
    for (i, m) in tailnet.machines.iter().enumerate() {
        println!("{:>3} {:<width$}  {}", i + 1, m.shown(), m.marks());
    }
    print!("Add which (number or name)? ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut typed = String::new();
    std::io::stdin().read_line(&mut typed).map_err(|e| Fail::Other(format!("can't read the choice: {e}")))?;
    let typed = typed.trim();
    if typed.is_empty() {
        return Err(Fail::Other("nothing added".into()));
    }
    match typed.parse::<usize>() {
        Ok(n) => {
            tailnet.machines.get(n.wrapping_sub(1)).ok_or_else(|| Fail::Usage(format!("no machine {n} in the list")))
        }
        Err(_) => tailnet.find(typed).map_err(|e| Fail::Api(ApiError::not_found(e))),
    }
}

/// Reach each host as a command would (starting a server there if `--autostart` allows).
fn check(cli: &Cli, list: &[Host], labels: &[String]) -> Result<(), Fail> {
    known(list, labels)?;
    let mut results = Vec::new();
    for h in list.iter().filter(|h| labels.is_empty() || labels.contains(&h.label)) {
        let answer = h.endpoint().map_err(Fail::Usage).and_then(|e| {
            let link = Link::open(&h.label, &e, cli.autostart).map_err(|e| Fail::Other(e.to_string()))?;
            Server::over(&h.label, link).connect(cli.autostart)?.call("server.status", json!({}))
        });
        let r = match answer {
            Ok(st) => json!({ "label": h.label, "ok": true, "version": st["version"], "sessions": st["sessions"] }),
            Err(f) => json!({ "label": h.label, "ok": false, "error": crate::commands::message(&f) }),
        };
        if !cli.json {
            match r["ok"] == true {
                true => println!("{}  ok  leon {}  {} session(s)", h.label, text(&r["version"]), r["sessions"]),
                false => println!("{}  failed  {}", h.label, text(&r["error"])),
            }
        }
        results.push(r);
    }
    if cli.json {
        print_json(&Value::Array(results.clone()));
    }
    match results.iter().all(|r| r["ok"] == true) {
        true => Ok(()),
        false => Err(Fail::Exit(4)),
    }
}

fn text(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}
