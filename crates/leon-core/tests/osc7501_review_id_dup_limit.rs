use leon_core::program_status::parse;

#[test]
fn id_limits_apply_to_every_duplicate_like_app() {
    let seg33 = "x".repeat(33);
    let deep9 = "a/b/c/d/e/f/g/h/i".to_string();
    let total129 = format!("{0}/{0}/{0}/{0}/x", "x".repeat(31));
    assert_eq!(total129.len(), 129);
    for bad in [seg33, deep9, total129] {
        let payload = format!("state=idle:id={bad}:id=ok");
        let got = parse(payload.as_bytes());
        assert!(got.is_err(), "over-limit id {bad:?} was accepted: {got:?}");
    }
}
