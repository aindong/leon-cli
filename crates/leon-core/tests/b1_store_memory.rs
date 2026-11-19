//! Scrollback B1, criterion 11: a pane's history costs about what its text costs. A test allocator
//! counts the bytes held: 10,000 lines of history must cost a terminal at most a fifth of what
//! they cost alacritty alone. Each side's cost is what it holds with the lines less what it holds
//! with none (the parser's 2 MB buffer for synchronized output, the screen).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use alacritty_terminal::event::VoidListener;
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use leon_core::terminal::{Spec, Terminal};
use leon_core::theme::DARK;

struct Counting;

static HELD: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        HELD.fetch_add(l.size() as isize, Ordering::SeqCst);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        HELD.fetch_sub(l.size() as isize, Ordering::SeqCst);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        HELD.fetch_add(new as isize - l.size() as isize, Ordering::SeqCst);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

struct Size;

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        24
    }
    fn screen_lines(&self) -> usize {
        24
    }
    fn columns(&self) -> usize {
        80
    }
}

/// 30,000 lines of 70 characters with a few colour changes, as the soak's agents print.
fn output() -> Vec<u8> {
    (0..30_000)
        .flat_map(|i| {
            format!("\x1b[32m{i:05}\x1b[0m agent says \x1b[1mthis\x1b[0m {}\r\n", "z".repeat(48)).into_bytes()
        })
        .collect()
}

/// The bytes a plain alacritty `Term` with 10,000 lines of history holds after `bytes`.
fn alacritty(bytes: &[u8]) -> isize {
    let start = HELD.load(Ordering::SeqCst);
    let mut term = Term::new(Config { scrolling_history: 10_000, ..Config::default() }, &Size, VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, bytes);
    let held = HELD.load(Ordering::SeqCst) - start;
    drop((term, parser));
    held
}

/// The bytes a terminal shown `bytes` holds once its threads have settled; it is kept, so its
/// memory isn't freed during a later measurement.
fn leon(bytes: &[u8], kept: &mut Vec<Terminal>) -> isize {
    let start = HELD.load(Ordering::SeqCst);
    let spec = Spec { command: vec!["/bin/sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_showing(spec, bytes, |_| {}).unwrap();
    // the reader thread makes its parser (2 MB) as it starts: wait until what is held stops moving
    let end = Instant::now() + Duration::from_secs(10);
    let (mut last, mut same) = (HELD.load(Ordering::SeqCst), 0);
    while same < 10 {
        assert!(Instant::now() < end, "the terminal's memory never settled");
        std::thread::sleep(Duration::from_millis(20));
        let now = HELD.load(Ordering::SeqCst);
        (last, same) = (now, if now == last { same + 1 } else { 0 });
    }
    let held = last - start;
    kept.push(t);
    held
}

#[test]
fn ten_thousand_lines_cost_at_most_a_fifth_of_what_they_cost_alacritty() {
    let bytes = output();
    let mut kept = Vec::new();
    leon(b"warm", &mut kept); // the first terminal allocates what every later one shares
    let alacritty = alacritty(&bytes) - alacritty(b"x");
    let leon_empty = leon(b"x", &mut kept);
    let leon_full = leon(&bytes, &mut kept);
    assert_eq!(kept[2].lines(0, 0, &DARK).history, 10_000);
    let leon = leon_full - leon_empty;
    eprintln!(
        "history of 10,000 lines: alacritty alone {alacritty} bytes, leon {leon} bytes ({:.1}% less); \
         an empty terminal {leon_empty} bytes",
        100. - 100. * leon as f64 / alacritty as f64
    );
    assert!(leon * 5 <= alacritty, "leon's history costs {leon} bytes, more than a fifth of alacritty's {alacritty}");
}
