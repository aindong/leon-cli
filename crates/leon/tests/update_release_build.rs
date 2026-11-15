//! Update criterion 1 (.agent/plans/2026-11-16-self-update-core.md): the trusted keys are built
//! in, and in a release build nothing changes them. The other update tests point a debug leon at
//! a throwaway key through `LEON_TEST_UPDATE`; this builds a `--release` leon and shows that it
//! ignores that variable and does not even contain its name.

mod release_kit;
mod update_kit;

use std::path::PathBuf;
use std::process::Command;

use update_kit::{assert_refused, run, Install, Release};

/// A `--release` leon without the app (the update code is the same, and gpui is most of a
/// release build's time), in its own target directory so it never replaces `target/release/leon`.
fn release_leon() -> PathBuf {
    // <target>/debug/leon -> <target>/update-release
    let target = PathBuf::from(env!("CARGO_BIN_EXE_leon")).parent().unwrap().parent().unwrap().join("update-release");
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .current_dir(release_kit::repo())
        // --offline: every crate was fetched to build this test
        .args(["build", "--locked", "--offline", "--release", "-p", "leon", "--no-default-features", "--target-dir"])
        .arg(&target)
        // this test's own cargo settings are for the debug build around it
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS");
    let o = run(&mut cargo, 45 * 60);
    assert_eq!(o.code, 0, "cargo build --release: {}", o.text());
    target.join("release/leon")
}

#[test]
fn a_release_build_ignores_the_test_seam() {
    let release = release_leon();
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());

    // the seam works where it is meant to: the debug leon trusts the throwaway key
    let o = leon.update(&rel, &srv, &["--check"]);
    assert_eq!(o.code, 10, "a debug build takes the throwaway key: {}", o.text());

    // the same setup with the release binary in the install's place
    // a new file, not new bytes in the old one: macOS remembers a file's signature by its inode
    std::fs::remove_file(&leon.exe).unwrap();
    std::fs::copy(&release, &leon.exe).unwrap();
    let before = leon.state();
    for args in [&[][..], &["--check"]] {
        let o = leon.update(&rel, &srv, args);
        assert_refused(&o, &["latest.json", "a key this leon does not trust"]);
    }
    leon.assert_unchanged(before, "a release build and a throwaway key");
    // and it is still the real version, whatever the seam says
    let seam =
        serde_json::json!({ "keys": rel.key.keys_file, "version": "0.0.1", "pause": "installing", "timeout_secs": 1 });
    let o = run(leon.cmd_with(&srv.url(), &seam).arg("self-update"), 60);
    assert_refused(&o, &["a key this leon does not trust"]);

    let binary = std::fs::read(&release).unwrap();
    let has = |needle: &[u8]| binary.windows(needle.len()).any(|w| w == needle);
    assert!(!has(b"LEON_TEST_UPDATE"), "a release build has no code that reads LEON_TEST_UPDATE");
    assert!(has(b"LEON_RELEASE_URL"), "the search finds names that are there");
    let debug = std::fs::read(env!("CARGO_BIN_EXE_leon")).unwrap();
    assert!(debug.windows(16).any(|w| w == b"LEON_TEST_UPDATE"), "and a debug build does read it");
}
