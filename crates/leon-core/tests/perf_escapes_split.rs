//! The escapes leon follows itself (X10 mouse, history clears) are found split at any byte, with
//! plenty of plain text around them, as the parse thread feeds them slice by slice.

use leon_core::terminal::Escapes;

/// Plain output with no escape in it, and newlines now and then.
fn plain(n: usize) -> Vec<u8> {
    (0..n).map(|i| if i % 80 == 79 { b'\n' } else { b'a' + (i % 26) as u8 }).collect()
}

/// Feeds `pieces` to a fresh scanner; per piece, the X10 state after it and whether a clear
/// completed in it.
fn run(pieces: &[&[u8]]) -> Vec<(bool, bool)> {
    let mut s = Escapes::default();
    pieces
        .iter()
        .map(|p| {
            let cleared = s.feed(p);
            (s.x10(), cleared)
        })
        .collect()
}

#[test]
fn each_escape_is_found_split_at_every_byte() {
    // (sequence, what comes before it, X10 after it, a history clear)
    let cases: [(&[u8], &[u8], bool, bool); 4] = [
        (b"\x1b[?9h", b"", true, false),
        (b"\x1b[?1000;9l", b"\x1b[?9h", false, false),
        (b"\x1b[3J", b"", false, true),
        (b"\x1bc", b"\x1b[?9h", false, true),
    ];
    let pad = plain(5000);
    for (seq, setup, x10, clear) in cases {
        let stream = [setup, &pad, seq, &pad].concat();
        let end = setup.len() + pad.len() + seq.len();
        assert_eq!(run(&[&stream]), [(x10, clear)], "{seq:?} unsplit");
        for at in 0..=stream.len() {
            let (a, b) = stream.split_at(at);
            let got = run(&[a, b]);
            assert_eq!(got[1].0, x10, "{seq:?} split at {at}: X10 at the end");
            // a clear is reported once, by the piece holding its last byte
            assert_eq!((got[0].1, got[1].1), (clear && at >= end, clear && at < end), "{seq:?} split at {at}");
            if at >= setup.len() {
                // nothing is decided mid-sequence
                let want = if at >= end { x10 } else { !setup.is_empty() };
                assert_eq!(got[0].0, want, "{seq:?} split at {at}: X10 after the first piece");
            }
        }
    }
}

#[test]
fn an_endless_parameter_is_still_dropped() {
    let mut s = Escapes::default();
    s.feed(&[b"\x1b[?".as_slice(), [b'1'; 100_000].as_slice()].concat());
    s.feed(b";9h");
    assert!(!s.x10(), "a parameter past the limit is dropped, not buffered");
}
