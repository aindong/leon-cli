//! M16b criterion 10: the README, architecture and performance docs cover the app's hosts.

fn doc(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn the_docs_cover_hosts_in_the_app() {
    let devbox = doc("docs/remote-hosts.md");
    for want in ["This Mac", "Add Host", "needs login", "leon hosts check"] {
        assert!(devbox.contains(want), "docs/remote-hosts.md covers {want:?}");
    }
    let arch =
        // the hosts decisions moved to docs/architecture/hosts.md (file-size limit)
        [
            "docs/architecture.md",
            "docs/architecture/protocol-and-clients.md",
            "docs/architecture/state-and-hosts.md",
            "docs/architecture/hosts.md",
        ]
        .map(doc)
            .concat();
    for want in ["leon_app::hosts", "needs login", "stale", "session.resume", "This Mac"] {
        assert!(arch.contains(want), "docs/architecture.md covers {want:?}");
    }
    let perf = doc("docs/performance.md");
    let section = perf.split("## M16b").nth(1).expect("a performance section for M16b");
    assert!(section.contains("through a link"), "the link number is recorded");
    assert!(section.contains(" ms"), "with its numbers");
    let bench = doc("crates/leon/benches/speed/attached.rs");
    assert!(bench.contains("Link::open") && bench.contains("LEON_SSH"), "the bench measures through a link");
}

/// The window-only screenshots: there, and the size of a window, not of a whole screen.
#[test]
fn the_screenshots_are_windows() {
    for name in ["m16b-hosts", "m16b-remote-pane", "m16b-unreachable"] {
        let path = format!("{}/../../docs/images/{name}.png", env!("CARGO_MANIFEST_DIR"));
        let png = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(&png[1..4], b"PNG", "{name} is a PNG");
        let (w, h) =
            (u32::from_be_bytes(png[16..20].try_into().unwrap()), u32::from_be_bytes(png[20..24].try_into().unwrap()));
        assert!(w >= 1000 && h >= 600, "{name} shows a whole window: {w}x{h}");
        let page = doc("docs/remote-hosts.md");
        assert!(page.contains(&format!("](images/{name}.png)")), "docs/remote-hosts.md shows {name}");
    }
}
