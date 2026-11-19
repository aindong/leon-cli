//! CI by what changed (.agent/plans/2026-11-18-i18n-setup.md): translations, images, brand and
//! community files start no run (ci.yml's paths-ignore), English docs alone run only the doc tests
//! on Linux, and anything else runs the whole gate. scripts/ci-changes.sh decides, in CI's first job.

mod help_walk;

use std::path::Path;
use std::process::Command;

fn ci() -> String {
    std::fs::read_to_string(help_walk::repo().join(".github/workflows/ci.yml")).expect("ci.yml exists")
}

fn git(dir: &Path, args: &[&str]) {
    let o = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid", "-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
}

/// What scripts/ci-changes.sh says for a commit that writes `files` on top of a first commit.
fn kind(files: &[&str]) -> String {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q"]);
    std::fs::write(d.path().join("first"), "").unwrap();
    git(d.path(), &["add", "-A"]);
    git(d.path(), &["commit", "-qm", "first"]);
    for f in files {
        let p = d.path().join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "x").unwrap();
    }
    git(d.path(), &["add", "-A"]);
    git(d.path(), &["commit", "-qm", "change"]);
    let o = Command::new("bash")
        .arg(help_walk::repo().join("scripts/ci-changes.sh"))
        .arg("HEAD^1")
        .current_dir(d.path())
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

#[test]
fn the_change_decides_the_jobs() {
    let none = ["docs/i18n/ja/install.md", "docs/images/a.png", "brand/x.svg", "CONTRIBUTING.md"];
    assert_eq!(kind(&none), "none");
    let none = ["CODE_OF_CONDUCT.md", "SECURITY.md", ".github/ISSUE_TEMPLATE/bug.yml"];
    assert_eq!(kind(&none), "none");
    assert_eq!(kind(&[".github/pull_request_template.md"]), "none");
    assert_eq!(kind(&["docs/install.md", "docs/i18n/ja/install.md"]), "docs");
    assert_eq!(kind(&["README.md"]), "docs");
    assert_eq!(kind(&["docs/lua/keys.md", "CLAUDE.md"]), "docs");
    assert_eq!(kind(&["website/hooks.py", "website/requirements.txt"]), "docs");
    assert_eq!(kind(&["docs/install.md", "crates/leon/src/main.rs"]), "full");
    // Markdown that is code: the skill's guide is compiled in, scripts and workflows are code
    assert_eq!(kind(&["crates/leon/src/skill/guide.md"]), "full");
    assert_eq!(kind(&["scripts/demo-video/README.md"]), "full");
    assert_eq!(kind(&["docs/api-snapshot.json"]), "full");
    assert_eq!(kind(&["Cargo.toml"]), "full");
    assert_eq!(kind(&["docs/i18n/ja/install.md", ".github/workflows/ci.yml"]), "full");
}

#[test]
fn code_moved_into_the_docs_still_runs_everything() {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q"]);
    std::fs::create_dir_all(d.path().join("crates/leon/src")).unwrap();
    let lines: String = (1..50).map(|i| format!("line {i}\n")).collect();
    std::fs::write(d.path().join("crates/leon/src/notes.md"), lines).unwrap();
    git(d.path(), &["add", "-A"]);
    git(d.path(), &["commit", "-qm", "first"]);
    std::fs::create_dir_all(d.path().join("docs/i18n")).unwrap();
    git(d.path(), &["mv", "crates/leon/src/notes.md", "docs/i18n/notes.md"]);
    git(d.path(), &["commit", "-qm", "move"]);
    let o = Command::new("bash")
        .arg(help_walk::repo().join("scripts/ci-changes.sh"))
        .arg("HEAD^1")
        .current_dir(d.path())
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "full", "a rename is both its paths");
}

/// CI runs these scripts directly: without the executable bit the step would fail.
#[test]
fn the_scripts_ci_runs_are_executable() {
    use std::os::unix::fs::PermissionsExt;
    for s in ["ci-changes.sh", "linux-doc-tests.sh", "i18n-check.sh", "i18n-status.sh"] {
        let mode = std::fs::metadata(help_walk::repo().join("scripts").join(s)).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0, "scripts/{s} is executable");
    }
}

#[test]
fn an_unreadable_diff_runs_everything() {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q"]);
    for base in ["", "0000000000000000000000000000000000000000", "HEAD^1"] {
        let o = Command::new("bash")
            .arg(help_walk::repo().join("scripts/ci-changes.sh"))
            .arg(base)
            .current_dir(d.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "full", "base {base:?}");
    }
}

/// The lines under `paths-ignore:`, as written (quotes off).
fn paths_ignore(ci: &str) -> Vec<String> {
    let on = ci.split_once("\njobs:\n").unwrap().0;
    let lists: Vec<Vec<String>> = on
        .split("paths-ignore:\n")
        .skip(1)
        .map(|rest| {
            rest.lines()
                .take_while(|l| l.trim_start().starts_with("- ") || l.trim_start().starts_with('#'))
                .filter(|l| l.trim_start().starts_with("- "))
                .map(|l| l.trim_start().trim_start_matches("- ").trim_matches('"').to_string())
                .collect()
        })
        .collect();
    assert_eq!(lists.len(), 2, "push and pull_request each have paths-ignore: {on}");
    assert_eq!(lists[0], lists[1], "push and pull_request ignore the same paths");
    lists[0].clone()
}

#[test]
fn paths_ignore_is_the_scripts_list() {
    let want = [
        "docs/i18n/**",
        "docs/images/**",
        "brand/**",
        "CONTRIBUTING.md",
        "CODE_OF_CONDUCT.md",
        "SECURITY.md",
        ".github/ISSUE_TEMPLATE/**",
        ".github/pull_request_template.md",
    ];
    assert_eq!(paths_ignore(&ci()), want);
    let script = std::fs::read_to_string(help_walk::repo().join("scripts/ci-changes.sh")).unwrap();
    assert!(script.contains(&format!("SKIP=\"{}\"", want.join(" "))), "ci-changes.sh skips the same list");
}

/// The text of the job `id` under `jobs:`, up to the next job (a line `  <id>:`).
fn job(ci: &str, id: &str) -> String {
    let body = ci.split_once("\njobs:\n").unwrap().1;
    let is_job = |l: &str| l.starts_with("  ") && !l.starts_with("   ") && !l.starts_with("  #") && l.ends_with(':');
    let mut lines = body.lines().skip_while(|l| *l != format!("  {id}:"));
    assert!(lines.next().is_some(), "a job {id}");
    lines.take_while(|l| !is_job(l)).map(|l| format!("{l}\n")).collect()
}

#[test]
fn the_full_jobs_wait_for_the_verdict_and_docs_alone_run_the_doc_tests_on_linux() {
    let ci = ci();
    let changes = job(&ci, "changes");
    assert!(changes.contains("scripts/ci-changes.sh"), "{changes}");
    assert!(changes.contains("kind: ${{ steps.kind.outputs.kind }}"), "{changes}");
    // assigned first, so a failing script fails the step rather than leaving kind empty
    assert!(changes.contains("kind=$(scripts/ci-changes.sh \"$base\")"), "{changes}");
    assert!(!changes.contains("echo \"kind=$("), "{changes}");
    for id in ["macos", "linux"] {
        let j = job(&ci, id);
        assert!(j.contains("    needs: changes\n"), "{id} waits for changes: {j}");
        assert!(j.contains("    if: needs.changes.outputs.kind == 'full'\n"), "{id} runs on full only: {j}");
    }
    let docs = job(&ci, "docs");
    assert!(docs.contains("    needs: changes\n"), "{docs}");
    assert!(docs.contains("    if: needs.changes.outputs.kind == 'docs'\n"), "{docs}");
    assert!(docs.contains("    runs-on: ubuntu-24.04\n"), "{docs}");
    for step in ["- run: scripts/i18n-check.sh", "- run: scripts/linux-doc-tests.sh"] {
        assert!(docs.contains(step), "the docs job runs {step}: {docs}");
    }
    assert!(!docs.contains("macos") && !docs.contains("package-linux"), "no macOS, no packaging: {docs}");
}

/// The doc job's tests: the ones that read the docs, the README or the API snapshot, never the app's.
#[test]
fn the_doc_tests_are_the_ones_that_read_the_docs() {
    let o =
        Command::new("bash").arg(help_walk::repo().join("scripts/linux-doc-tests.sh")).arg("--list").output().unwrap();
    assert!(o.status.success());
    let list = String::from_utf8(o.stdout).unwrap();
    for t in [
        "crates/leon/tests/pub_doc_links.rs",
        "crates/leon/tests/pub_readme.rs",
        "crates/leon/tests/m13_privacy.rs",
        "crates/leon/tests/m7b_doc_commands.rs",
        "crates/leon/tests/m7b_lua_doc.rs",
        "crates/leon/tests/m7b_cli_doc.rs",
        "crates/leon/tests/release_docs.rs",
        "crates/leon/tests/gpui_adr.rs",
        "crates/leon-server/tests/m18_api_docs.rs",
        "crates/leon-server/tests/m18_snapshot_field_names.rs",
    ] {
        assert!(list.lines().any(|l| l == t), "{t} is a doc test: {list}");
    }
    assert!(!list.contains("leon-app"), "the app's tests are macOS only: {list}");
}
