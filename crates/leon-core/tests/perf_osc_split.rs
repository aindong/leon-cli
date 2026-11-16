//! The OSC 7501 scanner hands out the same pieces however its input is split, with plenty of plain
//! text around each sequence (the case the ESC search's speed-up must not break).

use leon_core::program_status::{Piece, Scanner};

#[derive(Debug, PartialEq)]
enum Out {
    Bytes(Vec<u8>),
    Report(Vec<u8>),
    Prompt,
    Reset,
    Pwd(Vec<u8>),
}

/// Feeds `chunks` in order, merging adjacent pass-through bytes and noting each OSC 7 taken.
fn scan(chunks: &[&[u8]]) -> Vec<Out> {
    let mut scanner = Scanner::default();
    let mut out = Vec::new();
    for chunk in chunks {
        scanner.feed(chunk, |piece| match piece {
            Piece::Bytes(b) => match out.last_mut() {
                Some(Out::Bytes(prev)) => prev.extend_from_slice(b),
                _ => out.push(Out::Bytes(b.to_vec())),
            },
            Piece::Report(p) => out.push(Out::Report(p.to_vec())),
            Piece::Prompt => out.push(Out::Prompt),
            Piece::Reset => out.push(Out::Reset),
        });
        if let Some(pwd) = scanner.take_pwd() {
            out.push(Out::Pwd(pwd));
        }
    }
    out
}

#[test]
fn each_sequence_is_found_split_at_every_byte() {
    let pad: Vec<u8> = (0..5000).map(|i| if i % 80 == 79 { b'\n' } else { b'a' + (i % 26) as u8 }).collect();
    let sequences: [&[u8]; 6] = [
        b"\x1b]7501;state=working\x1b\\",
        b"\x1b]7501;?\x07",
        b"\x1b]7;file://host/tmp/x\x07",
        b"\x1b]133;A\x1b\\",
        b"\x1bc",
        b"\x1b[1;31mred\x1b[0m",
    ];
    for seq in sequences {
        let stream = [&pad, seq, &pad].concat();
        let whole = scan(&[&stream]);
        assert!(whole.len() > 1 || seq.starts_with(b"\x1b[1"), "{seq:?}: the sequence is seen at all: {whole:?}");
        for at in 0..=stream.len() {
            let (a, b) = stream.split_at(at);
            let mut split = scan(&[a, b]);
            // a pwd is taken after the piece that completes it; place it as the unsplit run does
            if let Some(i) = split.iter().position(|o| matches!(o, Out::Pwd(_))) {
                let pwd = split.remove(i);
                split.push(pwd);
            }
            // bytes handed out on either side of the split merge into one run
            let mut merged: Vec<Out> = Vec::new();
            for o in split {
                match (merged.last_mut(), o) {
                    (Some(Out::Bytes(prev)), Out::Bytes(b)) => prev.extend_from_slice(&b),
                    (_, o) => merged.push(o),
                }
            }
            assert_eq!(merged, whole, "{seq:?} split at {at}");
        }
    }
}

/// xorshift64: deterministic, so a failure reproduces from the printed case.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const TOKENS: &[&[u8]] = &[
    b"\x1b",
    b"\x1b",
    b"]",
    b"c",
    b"\\",
    b"[",
    b"7501;",
    b"7;",
    b"133;A",
    b"state=working",
    b"?",
    b"\x07",
    b"\x18",
    b"\x1b\\",
    b"\x1b[31m",
    b"\x1b[0m",
    b"\x1b[1;92m",
    b"\x1bc",
    b"\x1b]7;file://h/x\x07",
    b"\x1b]7501;state=done\x07",
    b"\x1b]133;A\x1b\\",
    b"\x1b]0;title\x07",
    b"plain text ",
    b"\n",
];

/// Feeding one byte at a time never skips ahead (the skip needs the next byte in the same input),
/// so it is the scanner's per-byte path: any split must give what it gives.
#[test]
fn random_splits_give_what_one_byte_at_a_time_gives() {
    let mut rng = Rng(0x7501_5ca7);
    for case in 0..2000 {
        let mut stream = Vec::new();
        for _ in 0..rng.below(300) {
            if rng.below(8) == 0 {
                stream.push(rng.next() as u8);
            } else {
                stream.extend_from_slice(TOKENS[rng.below(TOKENS.len())]);
            }
        }
        let bytes: Vec<&[u8]> = stream.chunks(1).collect();
        let mut cuts: Vec<usize> = (0..rng.below(6)).map(|_| rng.below(stream.len() + 1)).collect();
        cuts.extend([0, stream.len()]);
        cuts.sort();
        let pieces: Vec<&[u8]> = cuts.windows(2).map(|w| &stream[w[0]..w[1]]).collect();
        let pwds = |out: &[Out]| out.iter().filter(|o| matches!(o, Out::Pwd(_))).count();
        let (slow, fast) = (scan(&bytes), scan(&pieces));
        // without the pwds, bytes on either side of one merge
        let strip = |out: Vec<Out>| {
            let mut merged: Vec<Out> = Vec::new();
            for o in out.into_iter().filter(|o| !matches!(o, Out::Pwd(_))) {
                match (merged.last_mut(), o) {
                    (Some(Out::Bytes(prev)), Out::Bytes(b)) => prev.extend_from_slice(&b),
                    (_, o) => merged.push(o),
                }
            }
            merged
        };
        // a pwd is noted per feed call: the byte-at-a-time run notes each, a piece only its last
        assert!(pwds(&fast) <= pwds(&slow), "case {case}: {stream:?}");
        assert_eq!(strip(fast), strip(slow), "case {case}: {stream:?}");
    }
}
