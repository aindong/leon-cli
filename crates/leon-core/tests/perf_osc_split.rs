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
