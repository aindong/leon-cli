//! Update criterion 14 (.agent/plans/2026-11-16-self-update-core.md): `leon self-update --check`
//! reports and changes nothing, and its exit status says which: 0 up to date, 10 a newer release.

mod release_kit;
mod update_kit;

use serde_json::json;
use update_kit::{run, Install, Release, OLD, VERSION};

#[test]
fn check_reports_a_newer_release_with_exit_10_and_changes_nothing() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let o = leon.update(&rel, &srv, &["--check"]);
    assert_eq!(o.code, 10, "exit 10 for a newer release: {}", o.text());
    assert!(o.out.contains(&format!("leon {VERSION} is available; you have {OLD}")), "{}", o.text());
    assert!(o.out.contains("leon self-update"), "it says how to get it: {}", o.text());
    assert_eq!(o.err, "", "a newer release is not an error");
    leon.assert_unchanged(before, "--check");
    assert_eq!(srv.asked(), ["latest.json", "latest.json.minisig"], "--check downloads no release file");
}

#[test]
fn check_reports_up_to_date_with_exit_0() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let seam = json!({ "keys": rel.key.keys_file, "version": VERSION });
    let o = run(leon.cmd_with(&srv.url(), &seam).args(["self-update", "--check"]), 60);
    assert_eq!(o.code, 0, "exit 0 when up to date: {}", o.text());
    assert!(o.out.contains(&format!("leon {VERSION} is up to date")), "{}", o.text());
}

#[test]
fn the_help_lists_the_exit_status_and_the_flag() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let help = run(leon.cmd(&rel, &srv).arg("--help"), 30).out;
    let footer = help.split_once("Exit codes:").expect("an exit-codes footer").1;
    let line = footer.lines().find(|l| l.trim_start().starts_with("10 ")).expect("exit code 10 is in the footer");
    assert!(line.contains("self-update --check") && line.contains("newer release"), "{line}");
    let own = run(leon.cmd(&rel, &srv).args(["self-update", "--help"]), 30).out;
    assert!(own.contains("--check") && own.contains("10"), "self-update's help has --check and its status: {own}");
}

#[test]
fn self_update_is_about_this_machine() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let o = run(leon.cmd(&rel, &srv).args(["-H", "elsewhere", "self-update"]), 30);
    assert_eq!(o.code, 2, "-H is a usage error: {}", o.text());
    assert!(o.err.contains("doesn't take -H"), "{}", o.err);
    assert_eq!(srv.requests(), Vec::<String>::new(), "and nothing was asked");
}
