//! The owner's other machines on the tailnet, for the app's Connect menu: tailscaled's LocalAPI
//! `status` with its peers, read only (docs/architecture/tailnet.md). A machine is listed only
//! when `tailnet://` would let leon reach it: signed in as this machine's user, untagged, and not
//! this machine. Whether one serves leon isn't asked (no port is probed): choosing it says.

use serde_json::Value;

use crate::localapi::{self, Me};

/// A tailnet of thousands of machines is a few MB of status.
const MOST_BYTES: usize = 8 << 20;
const MOST_PEERS: usize = 256;

/// One of the owner's machines.
#[derive(Debug, Clone, PartialEq)]
pub struct Owned {
    /// Its MagicDNS name, lower case, without the trailing dot.
    pub dns: String,
    pub online: bool,
}

impl Owned {
    /// Its short name: the MagicDNS name's first label.
    pub fn name(&self) -> &str {
        self.dns.split('.').next().unwrap_or_default()
    }
}

/// The owner's other machines, online ones first, then by name.
pub fn owned() -> Result<Vec<Owned>, String> {
    parse(&localapi::get("/localapi/v0/status", MOST_BYTES)?).map(|(_, o)| o)
}

/// A plain DNS name (letters, digits, `-` and `.`, at most 253), lower case and without the dot.
fn plain(dns: &str) -> Option<String> {
    let d = dns.trim_end_matches('.').to_ascii_lowercase();
    let ok = !d.is_empty() && d.len() <= 253 && d.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.');
    ok.then_some(d)
}

/// This machine and the owner's other machines in a `status` reply. A status that isn't running
/// or doesn't say who this machine is is an error, as `parse_status`'s; a peer with any field
/// missing or of the wrong type is left out.
pub fn parse(body: &[u8]) -> Result<(Me, Vec<Owned>), String> {
    let me = localapi::parse_status(body)?;
    let v: Value = serde_json::from_slice(body).map_err(|_| "can't read Tailscale's status: not JSON".to_string())?;
    let peers = v.get("Peer").and_then(Value::as_object).into_iter().flat_map(|p| p.values());
    let mut out: Vec<Owned> = peers
        .filter(|p| p.get("UserID").and_then(Value::as_u64) == Some(me.user))
        .filter(|p| p.get("ID").and_then(Value::as_str).is_some_and(|id| !id.is_empty() && id != me.node))
        // untagged: Tailscale leaves Tags out (or null) for a machine of a person's
        .filter(|p| matches!(p.get("Tags"), None | Some(Value::Null)))
        .filter_map(|p| {
            let dns = plain(p.get("DNSName")?.as_str()?)?;
            let online = p.get("Online").and_then(Value::as_bool).unwrap_or(false);
            (Some(&dns) != me.dns.as_ref()).then_some(Owned { dns, online })
        })
        .collect();
    out.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.dns.cmp(&b.dns)));
    out.dedup_by(|a, b| a.dns == b.dns);
    out.truncate(MOST_PEERS);
    Ok((me, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS: &str = r#"{"BackendState":"Running",
        "Self":{"ID":"nself","UserID":7,"DNSName":"mac.tail.ts.net.","TailscaleIPs":["100.64.0.1"]},
        "User":{"7":{"ID":7,"LoginName":"owner@example.com"},"8":{"ID":8,"LoginName":"other@example.com"}},
        "Peer":{
          "k1":{"ID":"n1","UserID":7,"DNSName":"devbox.tail.ts.net.","Online":true},
          "k2":{"ID":"n2","UserID":7,"DNSName":"Laptop.tail.ts.net.","Online":false,"Tags":null},
          "k3":{"ID":"n3","UserID":7,"DNSName":"ci.tail.ts.net.","Online":true,"Tags":["tag:ci"]},
          "k4":{"ID":"n4","UserID":8,"DNSName":"theirs.tail.ts.net.","Online":true},
          "k5":{"ID":"nself","UserID":7,"DNSName":"mac.tail.ts.net.","Online":true},
          "k6":{"ID":"n6","UserID":7,"DNSName":"bad name;rm.tail.ts.net.","Online":true},
          "k7":{"ID":"n7","UserID":"7","DNSName":"stringid.tail.ts.net.","Online":true},
          "k8":{"ID":"n8","UserID":7,"Online":true},
          "k9":{"ID":"n9","UserID":7,"DNSName":"tagbad.tail.ts.net.","Tags":"tag:x"},
          "k10":5}}"#;

    #[test]
    fn only_the_owners_other_untagged_machines() {
        let (me, owned) = parse(STATUS.as_bytes()).unwrap();
        assert_eq!(me.login, "owner@example.com");
        let names: Vec<(&str, bool)> = owned.iter().map(|o| (o.dns.as_str(), o.online)).collect();
        assert_eq!(names, [("devbox.tail.ts.net", true), ("laptop.tail.ts.net", false)]);
        assert_eq!(owned[0].name(), "devbox");
    }

    #[test]
    fn a_status_that_isnt_running_is_an_error() {
        assert!(parse(br#"{"BackendState":"Stopped"}"#).is_err());
        assert!(parse(b"not json").is_err());
        let none = r#"{"BackendState":"Running","Self":{"ID":"n","UserID":7},"User":{"7":{"LoginName":"o"}}}"#;
        assert_eq!(parse(none.as_bytes()).unwrap().1, []);
    }
}
