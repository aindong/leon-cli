//! `tailnet://` endpoints: a plain TCP connection to a server that serves over Tailscale
//! (docs/architecture/tailnet.md). WireGuard encrypts it, so it may only go to a Tailscale
//! address, and only to one of this user's own untagged machines, which the local tailscaled
//! vouches for before a byte is sent.

use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

use crate::localapi;

/// The port a server serves on unless its config says (`serve_tailscale_port`).
pub const PORT: u16 = 7297;
const CONNECT: Duration = Duration::from_secs(15);

/// Inside Tailscale's ranges: 100.64.0.0/10 or fd7a:115c:a1e0::/48.
pub fn in_ranges(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.octets()[0] == 100 && v4.octets()[1] & 0xc0 == 64,
        IpAddr::V6(v6) => v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
    }
}

/// A machine's own Tailscale address: in the ranges, but not Tailscale's DNS (100.100.100.100)
/// and not a 4via6 address (fd7a:115c:a1e0:b1a::/64, a LAN host behind a subnet router). Loopback
/// only under the tests' seam. An IPv4-mapped IPv6 address is none of these.
pub fn on_tailnet(ip: IpAddr) -> bool {
    if ip.is_loopback() {
        return localapi::loopback_is_tailnet();
    }
    let via = matches!(ip, IpAddr::V6(v6) if v6.segments()[..4] == [0xfd7a, 0x115c, 0xa1e0, 0x0b1a]);
    in_ranges(ip) && !via && ip != IpAddr::from([100, 100, 100, 100])
}

/// Connect to `host:port` over the tailnet: only to a Tailscale address that whois says is one of
/// this user's other untagged machines, and to that checked address, not the name again.
pub fn connect(host: &str, port: u16) -> Result<TcpStream, String> {
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("can't find {host}: {e}"))?
        .filter(|a| on_tailnet(a.ip()))
        .collect();
    if addrs.is_empty() {
        return Err(format!(
            "{host} isn't on the tailnet: tailnet:// connects only to a machine's Tailscale address (100.64.0.0/10 \
             or fd7a:115c:a1e0::/48)"
        ));
    }
    let me = localapi::status()?;
    let mut why = String::new();
    for a in addrs {
        match localapi::whois(a).and_then(|p| localapi::admit(&me, &p, &[])) {
            Err(e) => why = format!("won't connect to {host} ({}): {e}", a.ip()),
            Ok(()) => match TcpStream::connect_timeout(&a, CONNECT) {
                Ok(s) => return Ok(s),
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
                    why = format!(
                        "nothing answers on {host} port {port}: turn on serve_tailscale there (leon config set \
                         serve_tailscale true) and keep its server running (leon server start)"
                    )
                }
                Err(e) => why = format!("can't reach {host} port {port} over the tailnet: {e}"),
            },
        }
    }
    Err(why)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_machine_addresses_on_the_tailnet_count() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        for good in ["100.64.0.1", "100.127.255.254", "fd7a:115c:a1e0::1", "fd7a:115c:a1e0:ab12::5"] {
            assert!(on_tailnet(ip(good)), "{good}");
        }
        for bad in [
            "100.63.255.255",
            "100.128.0.1",
            "100.100.100.100",
            "fd7a:115c:a1e0:b1a::c0a8:1",
            "fd7a:115c:a1e1::1",
            "::ffff:100.64.0.1",
            "0.0.0.0",
            "::",
            "192.168.1.5",
            "10.0.0.2",
        ] {
            assert!(!on_tailnet(ip(bad)), "{bad}");
        }
        if !localapi::loopback_is_tailnet() {
            assert!(!on_tailnet(ip("127.0.0.1")) && !on_tailnet(ip("::1")), "loopback isn't the tailnet");
        }
    }
}
