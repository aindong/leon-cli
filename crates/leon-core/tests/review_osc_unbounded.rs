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

#[test]
fn an_overlong_osc_is_cancelled_not_cut_short() {
    let mut s = Scanner::default();
    let mut pass = Vec::new();
    let long = format!("a\x1b]2;{}\x07b\x1b]2;{}\x1b\\c", "x".repeat(5000), "y".repeat(5000));
    s.feed(long.as_bytes(), |p| {
        if let Piece::Bytes(b) = p {
            pass.extend_from_slice(b)
        }
    });
    let shown: Vec<u8> = pass.iter().copied().filter(|&b| b != b'x' && b != b'y').collect();
    assert_eq!(shown, b"a\x1b]2;\x18b\x1b]2;\x18c", "each OSC is cancelled, its terminator swallowed");
}
