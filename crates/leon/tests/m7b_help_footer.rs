//! M7b criterion 5 (.agent/plans/2026-11-14-m7b-ship.md): `leon --help` lists the exit codes, and an
//! unknown command suggests the nearest one.

use std::process::Command;

fn leon(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_leon")).args(args).env("LEON_AUTOSTART", "0").output().unwrap()
}

#[test]
fn help_footer_lists_exit_codes() {
    let text = String::from_utf8(leon(&["--help"]).stdout).unwrap();
    let footer = text.split_once("Exit codes:").unwrap_or_else(|| panic!("no exit-codes footer:\n{text}")).1;
    for (code, word) in
        [("0", "ok"), ("1", "fail"), ("2", "command line"), ("3", "target"), ("4", "server"), ("130", "interrupt")]
    {
        let line =
            footer.lines().find(|l| l.trim_start().starts_with(code) && l.trim_start()[code.len()..].starts_with(' '));
        let line = line.unwrap_or_else(|| panic!("exit code {code} not in the footer:\n{footer}"));
        assert!(line.to_lowercase().contains(word), "exit code {code} should mention {word:?}: {line}");
    }
}

#[test]
fn unknown_command_suggests_the_nearest() {
    for (typo, want) in [("sesion", "session"), ("captur", "capture"), ("splt", "split")] {
        let o = leon(&[typo]);
        assert_eq!(o.status.code(), Some(2), "leon {typo}");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains(&format!("'{want}'")), "leon {typo} should suggest {want}: {err}");
    }
}
