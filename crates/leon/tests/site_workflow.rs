//! The website's workflow (.agent/plans/2026-11-19-site-and-release-kit.md, criteria 7–9):
//! `.github/workflows/site.yml` deploys the latest release's docs with GitHub's own Pages actions,
//! pinned, with no more permissions than Pages needs; the media release can never be "latest"; and
//! CI builds the site strictly on a docs change. The pages themselves are website/test_site.py's.

mod help_walk;

use std::io::Write;
use std::process::{Command, Stdio};

fn read(path: &str) -> String {
    std::fs::read_to_string(help_walk::repo().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The text of the job `id` under `jobs:`, up to the next job (a line `  <id>:`).
fn job(yml: &str, id: &str) -> String {
    let body = yml.split_once("\njobs:\n").unwrap().1;
    let is_job = |l: &str| l.starts_with("  ") && !l.starts_with("   ") && !l.starts_with("  #") && l.ends_with(':');
    let mut lines = body.lines().skip_while(|l| *l != format!("  {id}:"));
    assert!(lines.next().is_some(), "a job {id}");
    lines.take_while(|l| !is_job(l)).map(|l| format!("{l}\n")).collect()
}

fn latest_tag(json: &str) -> (bool, String) {
    let mut child = Command::new("bash")
        .arg(help_walk::repo().join("website/latest-tag.sh"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(json.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    (o.status.success(), String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[test]
fn it_runs_after_the_release_workflow_and_by_hand_from_the_default_branch_only() {
    let yml = read(".github/workflows/site.yml");
    let on = yml.split_once("\non:\n").expect("an on: block").1.split("\n\n").next().unwrap();
    assert_eq!(on, "  workflow_run:\n    workflows: [Release]\n    types: [completed]\n  workflow_dispatch:");
    // a `release` or `push` trigger would run from a tag, which the github-pages environment would
    // have to let deploy: anyone who can push a tag could then publish site code nobody reviewed
    let build = job(&yml, "build");
    assert!(
        build.contains("github.event.workflow_run.conclusion == 'success'")
            && build.contains("github.event.workflow_run.event == 'push'"),
        "{build}"
    );
    // that Release run published: only a tag push publishes, a run by hand stops at a draft
    let release = read(".github/workflows/release.yml");
    let publish = release.split("- name: Publish it").nth(1).expect("release.yml publishes in a step of its own");
    assert!(
        publish.lines().nth(1).unwrap().contains("if: github.event_name == 'push' && github.ref_type == 'tag'"),
        "{publish}"
    );
}

#[test]
fn it_builds_the_latest_release_s_tag_and_checks_it_first() {
    let build = job(&read(".github/workflows/site.yml"), "build");
    let pick = build.find("releases/latest").expect("asks for releases/latest");
    let check = build.find("website/latest-tag.sh").expect("checks the tag");
    let checkout = build.find("ref: ${{ steps.tag.outputs.tag }}").expect("checks out the tag it found");
    assert!(pick < checkout && check < checkout, "the tag is checked before it is checked out: {build}");
    assert!(build.contains("LEON_SITE_ROOT") && build.contains("LEON_SITE_REF"), "{build}");
    assert!(build.contains("website/build.sh"), "{build}");

    assert_eq!(latest_tag(r#"{"tag_name":"v1.2.3","prerelease":false}"#), (true, "v1.2.3".into()));
    for bad in [
        r#"{"tag_name":"media"}"#,
        r#"{"tag_name":"v1.2.3-rc.1"}"#,
        r#"{"tag_name":"v1.2.3;x"}"#,
        "{}",
        "not json",
        r#"{"tag_name":"v1.2.3\nv1"}"#,
    ] {
        assert!(!latest_tag(bad).0, "refused: {bad}");
    }
}

#[test]
fn every_action_is_github_s_own_and_pinned_and_permissions_are_pages_only() {
    let yml = read(".github/workflows/site.yml");
    let uses: Vec<&str> =
        yml.lines().filter_map(|l| l.trim().trim_start_matches("- ").strip_prefix("uses: ")).collect();
    assert!(uses.iter().any(|u| u.starts_with("actions/upload-pages-artifact@")), "{uses:?}");
    assert!(uses.iter().any(|u| u.starts_with("actions/deploy-pages@")), "{uses:?}");
    for u in &uses {
        assert!(u.starts_with("actions/"), "only GitHub's own actions: {u}");
        let pin = u.split('@').nth(1).unwrap_or("").split(' ').next().unwrap();
        assert!(pin.len() == 40 && pin.bytes().all(|b| b.is_ascii_hexdigit()), "pinned by commit: {u}");
    }
    let mut granted = vec![];
    let mut lines = yml.lines().peekable();
    while let Some(l) = lines.next() {
        if l.trim() == "permissions: {}" {
            continue;
        }
        if l.trim() == "permissions:" {
            let indent = l.len() - l.trim_start().len();
            while let Some(p) = lines.next_if(|p| p.len() - p.trim_start().len() > indent) {
                granted.push(p.split('#').next().unwrap().trim().to_string());
            }
        } else {
            assert!(!l.trim_start().starts_with("permissions:"), "permissions are listed one per line: {l}");
        }
    }
    granted.sort();
    granted.dedup();
    assert_eq!(granted, ["contents: read", "id-token: write", "pages: write"], "{yml}");
    assert!(yml.contains("persist-credentials: false"), "checkouts keep no token");
    // the token only where gh needs it, not in the build's environment
    assert!(!job(&yml, "build").contains("    env:\n      GH_TOKEN"), "no job-wide GH_TOKEN");
}

#[test]
fn python_deps_are_pinned_with_hashes_and_installed_by_hash() {
    let req = read("website/requirements.txt");
    let pins: Vec<&str> = req.lines().filter(|l| !l.starts_with(' ') && !l.starts_with('#') && !l.is_empty()).collect();
    assert!(pins.iter().any(|p| p.starts_with("mkdocs-material==")), "{req}");
    for pin in &pins {
        assert!(pin.contains("==") && pin.ends_with(" \\"), "an exact version, hashes below: {pin}");
        let after = req.split(pin).nth(1).unwrap().lines().nth(1).unwrap_or("");
        assert!(after.trim_start().starts_with("--hash=sha256:"), "{pin} has a hash: {after}");
    }
    let build = read("website/build.sh");
    assert!(build.contains("--require-hashes") && build.contains("-r website/requirements.txt"), "{build}");
    assert!(build.contains("build --strict"), "{build}");
}

#[test]
fn the_media_release_is_never_latest() {
    // the updater reads only releases/latest, which skips prereleases
    let update = read("crates/leon-update/src/lib.rs");
    assert!(update.contains("https://github.com/aindong/leon-cli/releases/latest/download/latest.json"));
    let fetch = read("website/fetch-media.sh");
    assert!(fetch.contains("isPrerelease"), "fetch-media.sh refuses a media release that could be latest: {fetch}");
    let docs = read("docs/releases.md");
    assert!(
        docs.contains("--prerelease") && docs.contains("--latest=false"),
        "releases.md makes the media release a prerelease"
    );
    for script in ["release.sh", "release-manifest.sh", "release-verify.sh"] {
        assert!(!read(&format!("scripts/{script}")).contains("media"), "{script} knows nothing of the media release");
    }
}

#[test]
fn ci_builds_the_site_on_any_change_but_translations_alone() {
    let ci = read(".github/workflows/ci.yml");
    let site = job(&ci, "site");
    assert!(site.contains("    needs: changes\n"), "{site}");
    assert!(site.contains("    if: needs.changes.outputs.kind != 'none'\n"), "docs and code changes both: {site}");
    assert!(site.contains("    runs-on: ubuntu-24.04\n"), "{site}");
    assert!(site.contains("- run: website/build.sh"), "{site}");
    assert!(site.contains("- run: target/site-venv/bin/python website/test_site.py"), "{site}");
    assert!(site.contains("fetch-depth: 0"), "the tags, for the latest release's build: {site}");
    let release = read(".github/workflows/release.yml");
    assert!(!release.contains("site"), "release.yml doesn't start the site: site.yml follows it");
}
