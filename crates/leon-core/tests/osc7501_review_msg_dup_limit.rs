use base64::Engine as _;
use leon_core::program_status::parse;

#[test]
fn msg_decoded_limit_applies_to_every_duplicate() {
    let big = base64::engine::general_purpose::STANDARD.encode("m".repeat(2049));
    assert_eq!(big.len(), 2732);
    let payload = format!("state=done:msg={big}:msg=aGk=");
    let got = parse(payload.as_bytes());
    assert!(got.is_err(), "2049-byte msg was accepted: {got:?}");
}
