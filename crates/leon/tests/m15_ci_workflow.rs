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

#[test]
fn ci_runs_the_full_macos_gate_and_linux_on_both_arches() {
    let ci = std::fs::read_to_string(help_walk::repo().join(".github/workflows/ci.yml")).expect("ci.yml exists");
    let jobs = jobs(&ci);
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
