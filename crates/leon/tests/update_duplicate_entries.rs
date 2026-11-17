//! Update criterion 4 (.agent/plans/2026-11-16-self-update-core.md): a manifest with two entries
//! for the same kind, os and arch is refused whole, whichever platform the pair is for.

mod release_kit;
mod update_kit;

use update_kit::{assert_refused, Install, Release};

#[test]
fn two_entries_for_one_platform_refuse_the_manifest() {
    for (kind, os, arch) in [("cli", "linux", "x86_64"), ("app", "macos", "arm64"), ("deb", "linux", "arm64")] {
        let rel = Release::new();
        rel.edit(&format!(
            ".files += [.files[] | select(.kind == \"{kind}\" and .os == \"{os}\" and .arch == \"{arch}\") | .name += \".copy\"]"
        ));
        assert_eq!(rel.manifest()["files"].as_array().unwrap().len(), 9, "the fixture adds one entry");
        let (srv, leon) = (rel.serve(), Install::cli());
        let before = leon.state();
        for args in [&[][..], &["--check"]] {
            let o = leon.update(&rel, &srv, args);
            assert_refused(&o, &[&format!("two files for {kind} {os} {arch}")]);
        }
        leon.assert_unchanged(before, "a manifest with a repeated entry");
        assert!(srv.asked().iter().all(|n| n.starts_with("latest.json")), "no file was fetched: {:?}", srv.asked());
    }
}
