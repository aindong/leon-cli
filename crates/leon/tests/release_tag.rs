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

/// Releases are cut from main only: with `--on-main`, a tag on a commit that main does not hold is
/// refused. Run in a throwaway repository, with a copy of the script and a Cargo.toml of its own.
#[test]
fn a_tag_on_a_commit_that_is_not_on_main_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    std::fs::create_dir(repo.join("scripts")).unwrap();
    std::fs::copy(release_kit::repo().join("scripts/release-version.sh"), repo.join("scripts/release-version.sh"))
        .unwrap();
    std::fs::write(repo.join("Cargo.toml"), "[workspace.package]\nversion = \"1.2.3\"\n").unwrap();
    let git = |args: &str| release_kit::sh(repo, &format!("git -c user.name=t -c user.email=t@t.invalid {args}"));
    let on_main = || {
        let o = std::process::Command::new("bash")
            .args(["scripts/release-version.sh", "--on-main", "v1.2.3"])
            .current_dir(repo)
            .output()
            .unwrap();
        (o.status.success(), text(&o))
    };
    git("init -q -b dev");
    git("add -A");
    git("commit -q -m one");

    // no origin/main to compare with (a shallow checkout): refused, and it says what is missing
    let (ok, out) = on_main();
    assert!(!ok && out.contains("origin/main"), "without origin/main nothing can be checked: {out}");

    // main is at this commit: a release
    git("update-ref refs/remotes/origin/main HEAD");
    let (ok, out) = on_main();
    assert!(ok && out.trim() == "1.2.3", "{out}");

    // a commit that only dev has: not a release
    git("commit -q --allow-empty -m two");
    let (ok, out) = on_main();
    assert!(!ok && out.contains("not on main") && !out.contains("1.2.3\n"), "a dev-only commit was accepted: {out}");
    // without --on-main (a run started by hand) the same commit is fine
    let o = std::process::Command::new("bash").args(["scripts/release-version.sh"]).current_dir(repo).output().unwrap();
    assert!(o.status.success(), "{}", text(&o));

    // main moved on past the tagged commit: still a release of a commit main holds
    git("update-ref refs/remotes/origin/main HEAD");
    git("checkout -q --detach HEAD~1");
    let (ok, out) = on_main();
    assert!(ok, "an older commit of main is on main: {out}");
    // and the tag check still applies
    let o = std::process::Command::new("bash")
        .args(["scripts/release-version.sh", "--on-main", "v9.9.9"])
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(!o.status.success() && text(&o).contains("v1.2.3"), "{}", text(&o));
}
