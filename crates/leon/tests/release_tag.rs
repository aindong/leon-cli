//! Release pipeline criterion 1: a tag that isn't `v<the Cargo.toml version>` stops a release
//! before anything is built (`scripts/release-version.sh`, the workflow's first job).

mod release_kit;

use release_kit::{script, text, VERSION};

#[test]
fn the_version_is_printed_and_a_matching_tag_passes() {
    for args in [vec![], vec![format!("v{VERSION}")]] {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let o = script("release-version.sh", &args, &[]);
        assert!(o.status.success(), "{args:?}: {}", text(&o));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), VERSION, "{args:?} prints the bare version");
    }
}

#[test]
fn a_tag_that_is_not_the_version_is_refused() {
    for tag in ["v99.99.99", VERSION, &format!("v{VERSION}-rc.1"), &format!("V{VERSION}"), "v"] {
        let o = script("release-version.sh", &[tag], &[]);
        assert!(!o.status.success(), "{tag} is not v{VERSION}, but release-version.sh passed it");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains(tag) && err.contains(&format!("v{VERSION}")), "the error names both: {err}");
        assert!(o.stdout.is_empty(), "a refused tag prints no version: {}", text(&o));
    }
}
