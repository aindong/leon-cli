//! Release pipeline criterion 6: `.github/workflows/release.yml` runs on `v*` tags and by hand,
//! builds with the release scripts, signs and verifies before it creates the release, makes a
//! draft unless a tag was pushed, and skips the optional steps when their secret is missing.
//! (That a hand-started run really yields a draft is proven by running it; this reads the file.)

mod release_kit;

fn workflow() -> String {
    std::fs::read_to_string(release_kit::repo().join(".github/workflows/release.yml")).expect("release.yml exists")
}

/// The steps of the workflow: each starts at a `      - ` line.
fn steps(yml: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for line in yml.lines() {
        if line.starts_with("      - ") {
            out.push(String::new());
        } else if !line.is_empty() && !line.starts_with("       ") {
            out.push(String::new()); // a job's own line ends the step
        }
        if let Some(step) = out.last_mut() {
            step.push_str(line);
            step.push('\n');
        }
    }
    out
}

fn step<'a>(steps: &'a [String], needle: &str) -> &'a str {
    steps.iter().find(|s| s.contains(needle)).unwrap_or_else(|| panic!("a step with {needle:?}"))
}

#[test]
fn the_release_workflow_builds_signs_verifies_then_releases() {
    let yml = workflow();
    let on = yml.split_once("\non:\n").expect("an on: section").1.split_once("\njobs:\n").unwrap().0;
    assert!(on.contains("push:") && on.contains("tags:") && on.contains("\"v*\""), "runs on v* tags: {on}");
    assert!(on.contains("workflow_dispatch:"), "runs by hand: {on}");
    assert!(!on.contains("branches") && !on.contains("pull_request"), "and on nothing else: {on}");

    for runner in ["macos-latest", "macos-15-intel", "ubuntu-24.04", "ubuntu-24.04-arm"] {
        assert!(yml.contains(runner), "a job runs on {runner}");
    }
    // in the order the workflow has to do them
    let order = [
        "scripts/release-version.sh",
        "scripts/release.sh --linux none",
        "scripts/package-linux.sh",
        "scripts/release-manifest.sh --complete",
        "scripts/release-verify.sh --complete",
        "scripts/release-notes.sh",
        "gh release create",
        "--draft=false",
        "scripts/release-homebrew.sh",
    ];
    let at: Vec<usize> =
        order.iter().map(|s| yml.find(s).unwrap_or_else(|| panic!("the workflow runs `{s}`"))).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "the steps come in this order: {order:?} (offsets {at:?})");
}

#[test]
fn a_hand_started_run_stays_a_draft_and_only_one_job_can_write() {
    let yml = workflow();
    let steps = steps(&yml);
    assert!(step(&steps, "gh release create").contains("--draft"), "the release is created as a draft");
    let publish = step(&steps, "--draft=false");
    // a tag, not only a push: a branch push must never publish, even if someone adds that trigger
    let tag_push = "if: github.event_name == 'push' && github.ref_type == 'tag'";
    assert!(publish.contains(tag_push), "only a tag push publishes: {publish}");
    assert_eq!(yml.matches("--draft=false").count(), 1);
    let tap = yml.split("\n  homebrew:\n").nth(1).expect("a homebrew job");
    assert!(tap.contains(&format!("    {tag_push} && ")), "only a tag push touches the tap: {tap}");

    assert!(yml.contains("\npermissions:\n  contents: read\n"), "the default token only reads");
    assert_eq!(yml.matches("contents: write").count(), 1, "one job may write releases");
    assert_eq!(yml.matches("secrets.LEON_RELEASE_KEY").count(), 1, "the signing key reaches one step");
    assert!(step(&steps, "secrets.LEON_RELEASE_KEY").contains("scripts/release-manifest.sh"));
    for uses in yml.lines().filter_map(|l| l.trim().trim_start_matches("- ").strip_prefix("uses: ")) {
        let pin = uses.split('@').nth(1).unwrap_or("").split(' ').next().unwrap();
        assert!(pin.len() == 40 && pin.bytes().all(|b| b.is_ascii_hexdigit()), "pinned by commit: {uses}");
    }
}

#[test]
fn optional_steps_skip_with_a_notice_when_their_secret_is_missing() {
    let yml = workflow();
    let steps = steps(&yml);
    for (secret, has) in [("HOMEBREW_TAP_TOKEN", "HAS_TAP_TOKEN"), ("APPLE_DEVELOPER_ID", "HAS_DEVELOPER_ID")] {
        assert!(
            yml.contains(&format!("{has}: ${{{{ secrets.{secret} != '' }}}}")),
            "{has} says whether {secret} is set"
        );
        let skip = steps
            .iter()
            .find(|s| s.contains(&format!("if: env.{has} != 'true'")))
            .unwrap_or_else(|| panic!("a step for when {secret} is missing"));
        assert!(skip.contains("::notice::") && skip.contains(secret), "it says what was skipped: {skip}");
        assert!(skip.contains("docs/releases.md"), "and where the steps are: {skip}");
    }
    let bump = step(&steps, "scripts/release-homebrew.sh");
    assert!(bump.contains("if: env.HAS_TAP_TOKEN == 'true'"), "the tap is only touched with a token: {bump}");
}
