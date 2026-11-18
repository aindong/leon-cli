//! Agent orchestration criterion 9: the guide stays readable in one pass, fixes what a comparison
//! with another orchestrator's guide found, and its orchestration part prints on its own.

use std::process::Command;

fn show(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_leon")).args(["skill", "show"]).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn the_guide_and_its_orchestration_part() {
    let guide = show(&[]);
    assert!(guide.len() < 10_000, "the guide is {} bytes", guide.len());
    let front = guide.split("\n---\n").next().unwrap();
    assert!(front.contains("not"), "the description says when not to use leon: {front}");
    assert!(!guide.contains("when your leon has it"), "split is in this leon");
    assert!(guide.contains("-H HOST"), "documents -H");
    assert!(guide.contains("exit 4") || guide.contains("`4`: server unreachable"), "says what exit 4 means");
    assert!(guide.contains("leon capture") && guide.to_lowercase().contains("landed"), "check before repeating a send");
    assert!(guide.contains("leon skill show orchestration"), "points to the orchestration part");

    let orch = show(&["orchestration"]);
    for must in ["leon agent start", "leon inbox", "leon agent tell", "leon agent ls", "leon notify", "permission"] {
        assert!(orch.contains(must), "the orchestration part covers {must}");
    }
    assert!(orch.contains("leon block close"), "cleaning up");
    let bad = Command::new(env!("CARGO_BIN_EXE_leon")).args(["skill", "show", "nope"]).output().unwrap();
    assert_eq!(bad.status.code(), Some(2), "an unknown part is a bad command line");
}
