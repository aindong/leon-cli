use super::*;

fn emulator() -> Emu {
    let writer = Writer { tx: mpsc::channel().0, queued_replies: Arc::default() };
    let listener = Listener { writer, seen: Arc::default() };
    let config = Config { scrolling_history: WINDOW, ..Config::default() };
    Emu { term: Term::new(config, &Size { rows: ROWS, cols: COLS }, listener), store: Store::new(SCROLLBACK) }
}

/// A full history, then `CSI 3` ending a 4 KiB slice, and `J` and more lines in the next.
fn split_clear() -> (Vec<u8>, usize) {
    let mut b: Vec<u8> = (0..SCROLLBACK + 100).flat_map(|i| format!("{i}\r\n").into_bytes()).collect();
    b.extend(std::iter::repeat_n(b'\n', SLICE - (b.len() + 3) % SLICE));
    b.extend_from_slice(b"\x1b[3");
    let at = b.len();
    assert_eq!(at % SLICE, 0);
    b.push(b'J');
    b.extend((0..50).flat_map(|i| format!("after {i}\r\n").into_bytes()));
    (b, at)
}

/// The line count after feeding `pieces` in order, one `feed` call each.
fn count(pieces: &[&[u8]]) -> usize {
    let (mut term, mut parser, mut escapes, scrolled) =
        (emulator(), Processor::new(), Escapes::default(), AtomicUsize::new(0));
    for p in pieces {
        feed(&mut term, &mut parser, &mut escapes, p, &scrolled);
    }
    scrolled.load(Ordering::SeqCst)
}

#[test]
fn a_history_clear_split_across_slices_moves_the_count_past_anything_kept() {
    let (bytes, at) = split_clear();
    let before = count(&[&bytes[..at]]);
    // split by the slicing inside one call, and between two calls
    assert!(count(&[&bytes]) >= before + GAP, "split by a slice");
    assert!(count(&[&bytes[..at], &bytes[at..]]) >= before + GAP, "split between calls");
    // the same clear whole in one slice, for comparison
    assert!(count(&[&bytes[..at - 3], &bytes[at - 3..]]) >= before + GAP, "unsplit");
}
