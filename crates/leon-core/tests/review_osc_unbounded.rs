//! Review proof: one unterminated OSC must not reach vte (which buffers OSCs unbounded with `std`)
//! without bound.

use leon_core::program_status::{Piece, Scanner};

#[test]
fn an_unterminated_osc_is_not_passed_to_the_emulator_without_bound() {
    let mut s = Scanner::default();
    let mut passed = 0usize;
    let mut count = |p: Piece<'_>| {
        if let Piece::Bytes(b) = p {
            passed += b.len()
        }
    };
    s.feed(b"\x1b]0;", &mut count);
    let chunk = vec![b'x'; 8192];
    for _ in 0..128 {
        s.feed(&chunk, &mut count);
    }
    assert!(passed < 64 << 10, "{passed} bytes of one unterminated OSC reached vte");
}
