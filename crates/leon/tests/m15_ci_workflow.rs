//! M15 criterion 5 (.agent/plans/2026-11-15-m15-linux-server-cli.md): the CI workflow has a macOS
//! job with the full gate and Linux jobs on both arches running the server + CLI suite. Also when
//! it runs: once per push to a PR, and on `main` and `dev` (the why is in ci.yml's comments).

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
/// Each keeps its indent: a key moved out of its parent is a different workflow.
fn section<'a>(ci: &'a str, key: &str) -> Vec<&'a str> {
    ci.lines()
        .skip_while(|l| l.trim_end() != format!("{key}:"))
        .skip(1)
        .take_while(|l| l.starts_with(' ') || l.starts_with('#') || l.is_empty())
        .map(str::trim_end)
        .filter(|l| !l.is_empty() && !l.trim_start().starts_with('#'))
        .collect()
}

fn ci() -> String {
    std::fs::read_to_string(help_walk::repo().join(".github/workflows/ci.yml")).expect("ci.yml exists")
}

#[test]
fn a_push_to_a_pr_branch_starts_one_run() {
    let ci = ci();
    assert_eq!(section(&ci, "on"), ["  push:", "    branches: [main, dev]", "  pull_request:"], "the triggers: {ci}");
}

#[test]
fn a_new_push_cancels_the_prs_older_run_and_never_one_on_main_or_dev() {
    let ci = ci();
    assert_eq!(
        section(&ci, "concurrency"),
        [
            "  group: ${{ github.workflow }}-${{ github.ref }}-${{ github.event_name == 'pull_request' && 'pr' || github.run_id }}",
            "  cancel-in-progress: ${{ github.event_name == 'pull_request' }}",
        ],
        "the concurrency block: {ci}"
    );
}

#[test]
fn ci_runs_the_full_macos_gate_and_linux_on_both_arches() {
    let jobs = jobs(&ci());
    let mac = jobs.iter().find(|(on, _)| on.starts_with("macos-")).expect("a macOS job");
    let steps = [
        "cargo fmt --check",
        "cargo clippy --all-targets -- -D warnings",
        "cargo nextest run --profile ci --workspace",
        "cargo test --doc --no-fail-fast",
    ];
    for step in steps {
        assert!(mac.1.contains(step), "the macOS job runs {step}: {}", mac.1);
    }
    // a matrix job names its runners in the body, not in `runs-on`
    let linux: String = jobs.iter().filter(|(_, b)| b.contains("ubuntu-24.04")).map(|(_, b)| b.as_str()).collect();
    for runner in ["ubuntu-24.04", "ubuntu-24.04-arm"] {
        assert!(linux.contains(runner), "a Linux job runs on {runner}: {linux}");
    }
    assert!(linux.contains("scripts/linux-test.sh"), "the Linux jobs run scripts/linux-test.sh: {linux}");
}

/// The macOS job runs its test binaries in parallel with cargo-nextest: one exact release from its
/// fixed URL, checked against its sha256 before it is unpacked. Doc-tests, which nextest doesn't
/// run, have their own step after it, and nothing is retried.
#[test]
fn the_macos_tests_run_in_parallel_under_a_pinned_checked_runner_without_retries() {
    let jobs = jobs(&ci());
    let job = &jobs.iter().find(|(on, _)| on.starts_with("macos-")).expect("a macOS job").1;
    // the job without comments: a step commented out is a step that doesn't run
    let code: Vec<&str> = job
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .map(|l| l.split_once(" # ").map_or(l, |(c, _)| c).trim_end())
        .collect();
    let mac = &code.join("\n");
    let at = |s: &str| mac.find(s).unwrap_or_else(|| panic!("the macOS job has {s:?}: {mac}"));
    let url = "https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.150/\
               cargo-nextest-0.9.150-universal-apple-darwin.tar.gz";
    let sum = "82a759b4d218d38359424196ab244c470e8f7c7d149c814fbbeef4d67c3c301a";
    assert!(at(url) < at(sum) && at(sum) < at("shasum -a 256 -c"), "downloaded, then checked");
    assert!(at("shasum -a 256 -c") < at("tar -xzf"), "checked before it is unpacked");
    assert!(at("tar -xzf") < at("cargo nextest run"), "installed before it runs");
    assert!(at("cargo nextest run") < at("cargo test --doc"), "then the doc-tests");
    // the two test steps exactly: no filter, no second run of the tests
    let tests: Vec<&str> =
        code.iter().copied().filter(|l| l.contains("cargo test") || l.contains("cargo nextest")).collect();
    let want =
        ["      - run: cargo nextest run --profile ci --workspace", "        run: cargo test --doc --no-fail-fast"];
    assert_eq!(tests, want, "the macOS job's test steps");
    // the one condition: doc-tests run after a failed test too, as under cargo test
    let condition = |l: &&str| l.trim_start().trim_start_matches("- ").starts_with("if:");
    let conditions: Vec<&str> = code.iter().copied().filter(condition).collect();
    assert_eq!(conditions, ["      - if: ${{ !cancelled() }}"], "the macOS job's conditions");
    let doc = code.iter().position(|l| l.contains("cargo test --doc")).unwrap();
    assert!(condition(&code[doc - 1]), "the condition is the doc-test step's");
    assert!(!mac.contains("--retries"), "no retries: {mac}");
    for hides in ["continue-on-error", "|| true", "command -v"] {
        assert!(!mac.contains(hides), "nothing skips or excuses a step ({hides}): {mac}");
    }
    let config = std::fs::read_to_string(help_walk::repo().join(".config/nextest.toml")).expect("a nextest config");
    let settings: Vec<&str> = config.lines().map(|l| l.split('#').next().unwrap().trim()).collect();
    for line in ["retries = 0", "fail-fast = false"] {
        assert!(settings.contains(&line), "the config sets {line}: {config}");
    }
    let retries = settings.iter().filter(|l| l.starts_with("retries")).collect::<Vec<_>>();
    assert_eq!(retries, [&"retries = 0"], "no profile or override retries");
    assert!(!settings.iter().any(|l| l.starts_with("default-filter")), "no test is filtered out: {config}");
}

/// Every action is pinned to a commit, and the token can only read.
#[test]
fn every_action_is_pinned_by_commit_and_the_token_only_reads() {
    let ci = ci();
    for line in ci.lines().filter(|l| l.trim_start().starts_with("- uses:") || l.trim_start().starts_with("uses:")) {
        let pin = line.split_once('@').map(|(_, p)| p.split_whitespace().next().unwrap_or("")).unwrap_or("");
        let commit = pin.len() == 40 && pin.chars().all(|c| c.is_ascii_hexdigit());
        assert!(commit, "pinned by commit: {line}");
    }
    assert_eq!(section(&ci, "permissions"), ["  contents: read"]);
}
