//! M20b criterion 4: a drawer's slide lasts at most 120 ms, and Reduce Motion skips it.

use std::time::Duration;

use leon_app::floats::{slide, SLIDE};

#[test]
fn a_slide_rises_to_done_within_120_ms() {
    assert!(SLIDE <= Duration::from_millis(120));
    let mut last = 0.;
    for ms in 0..SLIDE.as_millis() as u64 {
        let s = slide(Duration::from_millis(ms), false).expect("still sliding");
        assert!((0. ..1.).contains(&s) && s >= last, "{ms} ms: {s} after {last}");
        last = s;
    }
    assert_eq!(slide(SLIDE, false), None, "done at 120 ms");
    assert_eq!(slide(Duration::from_secs(5), false), None);
}

#[test]
fn reduce_motion_never_slides() {
    for ms in [0, 1, 60, 119] {
        assert_eq!(slide(Duration::from_millis(ms), true), None);
    }
}
