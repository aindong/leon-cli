//! Review proof: an 8-bit CSI (U+009B) in pasted text can't end bracketed paste early.

use leon_core::keys::paste_bytes;

#[test]
fn bracketed_paste_drops_c1_controls() {
    let out = paste_bytes("x\u{9b}201~touch pwned", true);
    assert_eq!(out, b"\x1b[200~x201~touch pwned\x1b[201~");
    assert_eq!(paste_bytes("caf\u{e9}", true), "\x1b[200~caf\u{e9}\x1b[201~".as_bytes(), "other text is kept");
}
