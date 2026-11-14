//! `-H HOST` and `leon hosts` (docs/architecture.md, "Remote hosts").

use leon_core::ApiError;
use leon_hosts::{hosts, Endpoint, Host, Link};
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
    hosts::check_label(name).map_err(|_| Fail::Usage(format!("-H {name:?}: not a host label or ssh:// endpoint")))?;
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
            if cli.json {
                print_json(&serde_json::to_value(&list).expect("json"));
            }
            for h in list.iter().filter(|_| !cli.json) {
                println!("{}  {}", h.label, h.endpoints.join(" "));
            }
        }
        HostsCmd::Add { label, endpoints, icon } => {
            hosts::check_label(label).map_err(Fail::Usage)?;
            for e in endpoints {
                e.parse::<Endpoint>().map_err(Fail::Usage)?;
            }
            if list.iter().any(|h| h.label == *label) {
                return Err(Fail::Other(format!("there is a host {label:?} already; remove it first")));
            }
            list.push(Host { label: label.clone(), endpoints: endpoints.clone(), icon: icon.clone() });
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
