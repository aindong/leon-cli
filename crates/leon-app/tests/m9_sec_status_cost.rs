//! One poll's work must not grow with the size of a pane's (idle, invisible) status records.
mod m9_support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use gpui::TestAppContext;
use m9_support::*;

thread_local! { static BYTES: Cell<usize> = const { Cell::new(0) }; }
struct Counting;
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = BYTES.try_with(|b| b.set(b.get() + l.size()));
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = BYTES.try_with(|b| b.set(b.get() + n));
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

// 256 idle records, each msg 2046 bytes ("QUFB" x 682 = base64 of "AAA" x 682)
const FLOOD: &str = r"m=$(printf 'QUFB%.0s' $(seq 682)); for i in $(seq 256); do printf '\033]7501;state=idle:id=r%s:msg=%s\007' $i $m; done; echo r-sent";

#[gpui::test]
fn a_poll_does_not_copy_every_record(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    run_until(&t, FLOOD, "r-sent");
    assert!(eventually(|| t.program_status().len() == 256), "256 records stored");
    settle(&t);
    let _ = ws.read_with(cx, |ws, _| ws.signature());
    BYTES.with(|b| b.set(0));
    let _ = ws.read_with(cx, |ws, _| ws.signature());
    let n = BYTES.with(|b| b.get());
    assert!(n < 256 << 10, "one poll allocated {n} bytes for 256 idle records");
    s.app.stop();
}
