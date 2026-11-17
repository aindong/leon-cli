//! Update criterion 6 (.agent/plans/2026-11-16-self-update-core.md): a release that needs a newer
//! macOS or glibc than this machine has is refused, by comparing the parts as numbers, and the
//! line gives both versions.

mod release_kit;
mod update_kit;

use std::process::Command;

use update_kit::{assert_refused, Install, Release, VERSION};

/// This machine's macOS or glibc version, asked of the system rather than of leon.
fn machine() -> String {
    let o = if cfg!(target_os = "macos") {
        Command::new("sw_vers").arg("-productVersion").output().unwrap()
    } else {
        Command::new("getconf").arg("GNU_LIBC_VERSION").output().unwrap()
    };
    assert!(o.status.success(), "the system says its version");
    String::from_utf8(o.stdout).unwrap().split_whitespace().last().unwrap().to_string()
}

fn release(macos: &str, glibc: &str) -> Release {
    let rel = Release::new();
    rel.edit(&format!(".min_os = {{macos: \"{macos}\", linux_glibc: \"{glibc}\"}}"));
    rel
}

#[test]
fn a_release_for_a_newer_system_is_refused() {
    let rel = release("99.0", "99.0");
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let system = if cfg!(target_os = "macos") { "macOS" } else { "glibc" };
    for args in [&[][..], &["--check"]] {
        let o = leon.update(&rel, &srv, args);
        assert_refused(&o, &[&format!("needs {system} 99.0"), &format!("this machine has {}", machine())]);
    }
    leon.assert_unchanged(before, "a release for a newer system");
    assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no file was fetched: {:?}", srv.asked());
}

#[test]
fn the_floor_is_compared_as_numbers() {
    // as text "9.0" sorts after "26.1" and "2.9" after "2.36"; as numbers both are older
    let machine = machine();
    let major: u64 = machine.split('.').next().unwrap().parse().unwrap();
    let minor: u64 = machine.split('.').nth(1).unwrap_or("0").parse().unwrap();
    assert!(
        if cfg!(target_os = "macos") { major >= 10 } else { minor >= 10 },
        "the fixture fits this machine: {machine}"
    );
    let rel = release("9.0", "2.9");
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "an older floor lets the update through: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
}

#[test]
fn a_floor_that_sorts_lower_as_text_is_still_refused() {
    // as text "100.0" sorts before "26.1" and "2.100" before "2.36"; as numbers both are newer
    let rel = release("100.0", "2.100");
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let needs = if cfg!(target_os = "macos") { "needs macOS 100.0" } else { "needs glibc 2.100" };
    assert_refused(&leon.update(&rel, &srv, &[]), &[needs, &format!("this machine has {}", machine())]);
    leon.assert_unchanged(before, "a floor that only numbers get right");
}

#[test]
fn a_release_with_no_floor_for_this_system_goes_through() {
    let rel = Release::new();
    rel.edit(".min_os = {}");
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "no floor, no refusal: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
}
