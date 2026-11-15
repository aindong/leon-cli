//! M15 criterion 5 (.agent/plans/2026-11-15-m15-linux-server-cli.md): the CI workflow has a macOS
//! job with the full gate and Linux jobs on both arches running the server + CLI suite.

mod help_walk;

/// The text of each job under `jobs:`, keyed by its `runs-on`.
fn jobs(ci: &str) -> Vec<(String, String)> {
    let body = ci.split_once("\njobs:\n").expect("a jobs: section").1;
    let mut out: Vec<(String, String)> = vec![];
    for line in body.lines() {
        // a job id sits at two spaces of indent
        if line.starts_with("  ") && !line.starts_with("   ") && line.trim_end().ends_with(':') {
            out.push((String::new(), String::new()));
        } else if let Some(job) = out.last_mut() {
            if let Some(r) = line.trim().strip_prefix("runs-on:") {
                job.0 = r.trim().to_string();
            }
            job.1.push_str(line);
            job.1.push('\n');
        }
    }
    out
}

/// The lines under a top-level key such as `on:`, up to the next top-level key, without comments.
fn section<'a>(ci: &'a str, key: &str) -> Vec<&'a str> {
    ci.lines()
        .skip_while(|l| l.trim_end() != format!("{key}:"))
        .skip(1)
        .take_while(|l| l.starts_with(' ') || l.starts_with('#') || l.is_empty())
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

fn ci() -> String {
    std::fs::read_to_string(help_walk::repo().join(".github/workflows/ci.yml")).expect("ci.yml exists")
}

/// A push to a PR branch is one run (the `pull_request` one), not two; a merge into `dev` and a
/// release merge into `main` still start a run of their own.
#[test]
fn a_push_to_a_pr_branch_starts_one_run() {
    let ci = ci();
    assert_eq!(section(&ci, "on"), ["push:", "branches: [main, dev]", "pull_request:"], "the triggers: {ci}");
}

/// A newer push cancels the PR's older run. A run on `main` or `dev` is in a group of its own (its
/// run id), so no later run cancels it or takes its place while it waits.
#[test]
fn a_new_push_cancels_the_prs_older_run_and_never_one_on_main_or_dev() {
    let ci = ci();
    assert_eq!(
        section(&ci, "concurrency"),
        [
            "group: ${{ github.workflow }}-${{ github.ref }}-${{ github.event_name == 'pull_request' && 'pr' || github.run_id }}",
            "cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
        ],
        "the concurrency block: {ci}"
    );
}

#[test]
fn ci_runs_the_full_macos_gate_and_linux_on_both_arches() {
    let jobs = jobs(&ci());
    let mac = jobs.iter().find(|(on, _)| on.starts_with("macos-")).expect("a macOS job");
    for step in ["cargo fmt --check", "cargo clippy --all-targets -- -D warnings", "cargo test --no-fail-fast"] {
        assert!(mac.1.contains(step), "the macOS job runs {step}: {}", mac.1);
    }
    // a matrix job names its runners in the body, not in `runs-on`
    let linux: String = jobs.iter().filter(|(_, b)| b.contains("ubuntu-24.04")).map(|(_, b)| b.as_str()).collect();
    for runner in ["ubuntu-24.04", "ubuntu-24.04-arm"] {
        assert!(linux.contains(runner), "a Linux job runs on {runner}: {linux}");
    }
    assert!(linux.contains("scripts/linux-test.sh"), "the Linux jobs run scripts/linux-test.sh: {linux}");
}
