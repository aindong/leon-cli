//! How an overlay looks part way through showing or going (the palette and what it hosts, the find
//! bar, the Activity list, a notice, a key sequence's hint): faded, a little smaller and lower.
//! GPUI-free, like `layout_motion`, on the same clock (the workspace's `motion_now`).

use std::time::Duration;

/// How long an overlay takes to show: fast, then settling.
pub const OVERLAY: Duration = Duration::from_millis(140);

/// How long it takes to go: quicker, slow then fast.
pub const OVERLAY_OUT: Duration = Duration::from_millis(100);

/// How far an overlay is shrunk at the start of showing (or the end of going), and how far lower.
pub const SHRUNK: f32 = 0.96;
pub const LOWER: f32 = 8.;

/// An overlay part way: how opaque (0–1), how big (`SHRUNK`–1, around its centre) and how far below
/// its place (px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub opacity: f32,
    pub scale: f32,
    pub rise: f32,
}

impl Look {
    /// At its place, fully shown.
    pub const SHOWN: Look = Look { opacity: 1., scale: 1., rise: 0. };

    /// `shown` (0–1) of the way from gone to shown.
    fn at(shown: f32) -> Look {
        Look { opacity: shown, scale: SHRUNK + (1. - SHRUNK) * shown, rise: LOWER * (1. - shown) }
    }
}

/// An overlay `elapsed` into showing (`showing`) or going: how it looks, or None once that is over.
pub fn look(showing: bool, elapsed: Duration) -> Option<Look> {
    let length = if showing { OVERLAY } else { OVERLAY_OUT };
    if elapsed >= length {
        return None;
    }
    let t = elapsed.as_secs_f32() / length.as_secs_f32();
    Some(if showing { Look::at(1. - (1. - t).powi(3)) } else { Look::at(1. - t.powi(3)) })
}

/// How far into showing (or going) an overlay at `opacity` is: where a fade that turns starts, so
/// it carries on from the look it had instead of jumping.
pub fn elapsed_at(showing: bool, opacity: f32) -> Duration {
    let o = opacity.clamp(0., 1.);
    if showing {
        OVERLAY.mul_f32(1. - (1. - o).cbrt())
    } else {
        OVERLAY_OUT.mul_f32((1. - o).cbrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_turned_fade_starts_where_it_was() {
        for o in [0.1, 0.5, 0.9] {
            let showing = look(true, elapsed_at(true, o)).unwrap().opacity;
            let going = look(false, elapsed_at(false, o)).unwrap().opacity;
            assert!((showing - o).abs() < 0.01 && (going - o).abs() < 0.01, "{o}: {showing} {going}");
        }
    }

    #[test]
    fn short_eased_out_in_and_in_out() {
        assert!((120..=220).contains(&OVERLAY.as_millis()), "OVERLAY {OVERLAY:?}");
        assert!(OVERLAY_OUT <= OVERLAY, "going is no slower than showing");
        let quarter = |showing, length: Duration| look(showing, length / 4).unwrap().opacity;
        assert!(quarter(true, OVERLAY) > 0.25, "showing: past a quarter at a quarter of the time");
        assert!(quarter(false, OVERLAY_OUT) > 0.75, "going: little gone at a quarter of the time");
    }

    #[test]
    fn from_gone_to_shown() {
        let start = look(true, Duration::ZERO).unwrap();
        assert_eq!(start, Look { opacity: 0., scale: SHRUNK, rise: LOWER });
        assert_eq!(look(true, OVERLAY), None, "over: shown");
        let going = look(false, Duration::ZERO).unwrap();
        assert_eq!(going, Look::SHOWN, "going starts where it is");
        let late = look(false, OVERLAY_OUT * 9 / 10).unwrap();
        assert!(late.opacity < 0.5 && late.scale < 1. && late.rise > 0., "and fades, shrinks and sinks: {late:?}");
    }
}
