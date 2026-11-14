//! M16a security review: the link needs no forwarding, so it asks ssh for none, whatever
//! ~/.ssh/config says: no agent on the host for as long as leon is attached, and no
//! LocalForward/DynamicForward binding a local TCP port per link connection.

#[test]
fn the_link_asks_for_no_forwarding() {
    let e: leon_hosts::Endpoint = "ssh://me@box".parse().unwrap();
    let a = e.ssh_args(true);
    let head = &a[..a.iter().position(|x| x == "--").unwrap()];
    assert!(head.iter().any(|x| x == "-a"), "agent forwarding left to ssh_config: {head:?}");
    assert!(head.iter().any(|x| x == "-x"), "X11 forwarding left to ssh_config: {head:?}");
    assert!(head.iter().any(|x| x == "ClearAllForwardings=yes"), "ssh_config port forwards bind TCP: {head:?}");
}
