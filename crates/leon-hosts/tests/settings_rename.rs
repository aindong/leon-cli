//! Renaming a host (Settings › Remote Hosts) checks the new label as `leon hosts add` does.

use leon_hosts::hosts::{rename, Host};

fn host(label: &str) -> Host {
    serde_json::from_value(serde_json::json!({ "label": label, "endpoints": ["ssh://me@box"] })).unwrap()
}

#[test]
fn rename_checks_the_label_as_add_does() {
    let mut list = vec![host("devbox"), host("build")];
    rename(&mut list, "devbox", "dev2").unwrap();
    assert_eq!(list[0].label, "dev2");
    let before = list.clone();
    let taken = rename(&mut list, "dev2", "build").unwrap_err();
    assert!(taken.starts_with(leon_hosts::hosts::TAKEN), "{taken}");
    assert!(rename(&mut list, "dev2", "-bad").unwrap_err().contains("bad host label"));
    assert!(rename(&mut list, "nope", "x").unwrap_err().contains("no host"));
    assert_eq!(list, before, "a refused rename changes nothing");
    rename(&mut list, "dev2", "dev2").unwrap();
}
