//! Random bytes never panic, chunking never changes the result, and the store stays capped.

use leon_core::program_status::{Change, Tracker, MAX_RECORDS};

/// xorshift64: deterministic, so a failure reproduces from the printed seed.
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
    b"\x1b", b"]", b"7501;", b"7501", b"133;A", b";", b":", b"=", b"/", b"\x07", b"\x1b\\", b"\x1bc", b"\x18",
    b"\x1a", b"\x9d", b"\xff", b"state=working", b"state=blocked", b"state=clear", b"state=", b"id=", b"app=",
    b"title=", b"progress=99", b"kind=auth", b"?", b"a", b"b", b"aGk=", b" ", b"\n",
];

fn random_stream(rng: &mut Rng) -> Vec<u8> {
    let mut out = Vec::new();
    for _ in 0..rng.below(300) {
        if rng.below(4) == 0 {
            out.push(rng.next() as u8);
        } else {
            out.extend_from_slice(TOKENS[rng.below(TOKENS.len())]);
        }
    }
    out
}

fn run(chunks: &[&[u8]]) -> (Vec<u8>, Vec<Change>, Vec<u8>) {
    let mut t = Tracker::default();
    let (mut pass, mut changes, mut reply) = (Vec::new(), Vec::new(), Vec::new());
    for chunk in chunks {
        let fed = t.feed(chunk, |b| pass.extend_from_slice(b));
        changes.extend(fed.changes);
        reply.extend(fed.reply);
    }
    (pass, changes, reply)
}

#[test]
fn random_streams_are_chunking_invariant() {
    for seed in 1..3000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let stream = random_stream(&mut rng);
        let mut chunks = Vec::new();
        let mut rest = &stream[..];
        while !rest.is_empty() {
            let (a, b) = rest.split_at(1 + rng.below(rest.len().min(17)));
            chunks.push(a);
            rest = b;
        }
        assert_eq!(run(&chunks), run(&[&stream]), "seed {seed}");
    }
}

#[test]
fn store_never_exceeds_the_cap() {
    let mut rng = Rng(42);
    let mut t = Tracker::default();
    for _ in 0..5000 {
        let id: String = (0..1 + rng.below(3)).map(|_| format!("s{}", rng.below(40))).collect::<Vec<_>>().join("/");
        let state = ["working", "blocked", "idle", "done", "error", "clear"][rng.below(6)];
        let seq = format!("\x1b]7501;state={state}:id={id}\x07");
        t.feed(seq.as_bytes(), |_| {});
        if rng.below(50) == 0 {
            t.store.prompt();
        }
        assert!(t.store.len() <= MAX_RECORDS, "store holds {}", t.store.len());
    }
    assert!(t.store.len() > 64, "the random ids should have filled the store: {}", t.store.len());
}
