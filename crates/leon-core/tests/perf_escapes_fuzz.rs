//! The one-pass escape scanner finds exactly what the old passes found: the X10 mouse state of the
//! byte-at-a-time scanner, and every history clear a `windows()` search of the whole stream sees,
//! in random streams split at random.

use leon_core::terminal::Escapes;

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

/// The X10 scanner as it was before the one-pass scanner: a state per byte.
#[derive(Default)]
struct OldX10 {
    on: bool,
    state: u8, // 0 text, 1 ESC, 2 CSI, 3 params
    params: Vec<u8>,
}

impl OldX10 {
    fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state = match (self.state, b) {
                (_, 0x1b) => 1,
                (1, b'[') => 2,
                (1, b'c') => {
                    self.on = false;
                    0
                }
                (2, b'?') => {
                    self.params.clear();
                    3
                }
                (3, b'0'..=b'9' | b';') if self.params.len() < 64 => {
                    self.params.push(b);
                    3
                }
                (3, b'h' | b'l') => {
                    for p in self.params.split(|&c| c == b';') {
                        match p {
                            b"9" => self.on = b == b'h',
                            b"1000" | b"1001" | b"1002" | b"1003" => self.on = false,
                            _ => {}
                        }
                    }
                    0
                }
                _ => 0,
            };
        }
    }
}

/// Where each history clear the old search finds ends (one past its last byte).
fn clear_ends(stream: &[u8]) -> Vec<usize> {
    let mut ends = Vec::new();
    for c in [b"\x1b[3J".as_slice(), b"\x1bc"] {
        ends.extend(stream.windows(c.len()).enumerate().filter(|(_, w)| w == &c).map(|(i, _)| i + c.len()));
    }
    ends
}

const TOKENS: &[&[u8]] = &[
    b"\x1b",
    b"\x1b",
    b"\x1b",
    b"[",
    b"[",
    b"?",
    b"9",
    b"3",
    b"J",
    b"c",
    b"h",
    b"l",
    b";",
    b"0",
    b"1000",
    b"1003",
    b"1",
    b"\n",
    b"\r\n",
    b"plain text ",
    b"\x1b[31m",
    b"\x1b[0m",
    b"\x1b]7;file:///x\x07",
    b"\x1b[?9h",
    b"\x1b[?9l",
    b"\x1b[3J",
    b"\x1bc",
    b"\x1b[?1000;9h",
];

#[test]
fn the_one_pass_finds_what_the_old_passes_found() {
    let mut rng = Rng(0x7501_cafe);
    for case in 0..2000 {
        let mut stream = Vec::new();
        for _ in 0..rng.below(400) {
            if rng.below(8) == 0 {
                stream.push(rng.next() as u8);
            } else if rng.below(50) == 0 {
                // a long run of params, past the scanner's limit
                stream.extend(std::iter::repeat_n(b'9', 60 + rng.below(20)));
            } else {
                stream.extend_from_slice(TOKENS[rng.below(TOKENS.len())]);
            }
        }
        let mut cuts: Vec<usize> = (0..rng.below(8)).map(|_| rng.below(stream.len() + 1)).collect();
        cuts.extend([0, stream.len()]);
        cuts.sort();
        let ends = clear_ends(&stream);
        let (mut new, mut old) = (Escapes::default(), OldX10::default());
        for w in cuts.windows(2) {
            let piece = &stream[w[0]..w[1]];
            let cleared = new.feed(piece);
            old.feed(piece);
            let want = ends.iter().any(|&e| e > w[0] && e <= w[1]);
            assert_eq!(cleared, want, "case {case}: clear in {:?} of {stream:?}", w[0]..w[1]);
            assert_eq!(new.x10(), old.on, "case {case}: X10 after {:?} of {stream:?}", w[0]..w[1]);
        }
    }
}
