use leon_app::glass::{Glass, SEE_GAIN};
use leon_app::theme::{contrast, mix, Theme, DARK};

#[test]
fn a_grey_desktop_is_no_worse_than_white_or_black() {
    // grey text that reads 4.5:1 over both a pure white and a pure black desktop
    let t = Theme { fg: 0x767676, muted: 0x767676, bg: 0x000000, ..DARK };
    let need = contrast(t.fg, t.bg).min(4.5);
    let a = Glass::new(&t, 0.8, 0.).pane_alpha();
    let shown = (1. - SEE_GAIN * (1. - a)).max(0.);
    let worst = (0..=255u32).map(|v| contrast(t.fg, mix(v * 0x010101, t.bg, shown))).fold(f64::INFINITY, f64::min);
    assert!(worst >= need - 0.01, "pane alpha {a}: fg on a grey desktop {worst:.2} < {need:.2}");
}
