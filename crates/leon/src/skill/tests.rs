use super::*;

#[test]
fn the_stub_shares_the_guides_front_matter() {
    let s = stub();
    let front = |t: &str| t.split("\n---\n").next().map(str::to_string);
    assert_eq!(front(&s), front(GUIDE));
    assert!(s.lines().any(|l| l == MARKER));
    assert!(s.contains("leon skill show"));
    assert!(s.lines().any(|l| l == "name: leon"));
}

#[test]
fn overall_state() {
    use State::*;
    assert_eq!(overall(&[Current, Current]), Current);
    assert_eq!(overall(&[Current]), Current);
    assert_eq!(overall(&[Missing, Missing]), Missing);
    assert_eq!(overall(&[Current, Missing]), Outdated);
    assert_eq!(overall(&[Foreign, Current]), Foreign);
    assert_eq!(overall(&[Foreign, Unreadable]), Unreadable);
}
