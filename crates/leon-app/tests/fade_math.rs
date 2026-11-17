//! Fading the panes that don't have the focus, the GPUI-free part: the blend, each pane's target,
//! and the steps between targets, with the time passed in so nothing here waits.

use std::time::{Duration, Instant};

use leon_app::fade::{frame, targets, theme, Fades};
use leon_app::floats::SLIDE;
use leon_app::render::{BgRect, Frame, Span};
use leon_app::theme::{contrast, mix, BUILTIN, DARK, LIGHT};
use leon_lua::settings::FADE;

const S: f32 = 0.4;
const MOTION: &dyn Fn() -> bool = &|| false;
const REDUCED: &dyn Fn() -> bool = &|| true;

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

/// Each pane's fade, in the order given.
fn amounts<const N: usize>(f: &Fades, ids: [&str; N]) -> [f32; N] {
    ids.map(|id| f.amount(id))
}

/// Two panes at rest with the focus on `a`.
fn two(t0: Instant) -> Fades {
    let mut f = Fades::default();
    f.aim(targets(&[("a", false), ("b", false)], Some("a"), S), t0, MOTION);
    f
}

#[test]
fn a_blend_ends_where_it_says() {
    assert_eq!(mix(0xD8CEC2, 0x292521, 0.), 0xD8CEC2, "0 is the colour itself, so off draws today's pixels");
    assert_eq!(mix(0xD8CEC2, 0x292521, 1.), 0x292521);
    assert_eq!(mix(0x000000, 0xFFFFFF, 0.5), 0x808080);
    assert_eq!(mix(0xFF0000, 0x0000FF, 0.5), 0x800080, "each channel on its own");
}

#[test]
fn a_faded_theme_keeps_the_status_colours_and_moves_the_rest() {
    for t in [DARK, LIGHT] {
        assert_eq!(theme(&t, 0.), t, "not faded: the theme itself");
        let f = theme(&t, S);
        assert_eq!((f.working, f.done, f.blocked, f.error), (t.working, t.done, t.blocked, t.error));
        assert_eq!(f.chrome, t.chrome, "the window's own colour is where the rest heads");
        let moved = [
            ("fg", f.fg, t.fg),
            ("bg", f.bg, t.bg),
            ("cursor", f.cursor, t.cursor),
            ("hover", f.hover, t.hover),
            ("border", f.border, t.border),
            ("text", f.text, t.text),
            ("title", f.title, t.title),
            ("muted", f.muted, t.muted),
            ("accent", f.accent, t.accent),
            ("selection", f.selection, t.selection),
            ("found", f.found, t.found),
            ("link", f.link, t.link),
        ];
        for (name, faded, full) in moved {
            assert_eq!(faded, mix(full, t.chrome, S), "{name}");
            assert_ne!(faded, full, "{name} is drawn weaker");
        }
        assert_eq!(f.ansi, t.ansi.map(|c| mix(c, t.chrome, S)));
    }
}

#[test]
fn a_screens_own_colours_fade_the_same_way() {
    let t = DARK;
    let span = |col, fg| Span {
        col,
        cells: 1,
        text: "x".into(),
        fg,
        bold: false,
        italic: false,
        underline: false,
        strikeout: false,
    };
    let f = Frame {
        lines: 1,
        cols: 4,
        rows: vec![vec![span(0, t.fg), span(1, t.ansi[1])]],
        backgrounds: vec![BgRect { line: 0, col: 2, len: 2, color: t.ansi[4] }],
        ..Frame::default()
    };
    assert_eq!(frame(f.clone(), t.chrome, 0.), f, "not faded: the frame the server sent");
    let faded = frame(f.clone(), t.chrome, S);
    assert_eq!(faded.rows[0][0].fg, mix(t.fg, t.chrome, S), "plain text");
    assert_eq!(faded.rows[0][1].fg, mix(t.ansi[1], t.chrome, S), "an ANSI colour");
    assert_eq!(faded.backgrounds[0].color, mix(t.ansi[4], t.chrome, S), "a cell's background");
    for colour in [faded.rows[0][0].fg, faded.rows[0][1].fg, faded.backgrounds[0].color] {
        assert!(![t.fg, t.ansi[1], t.ansi[4]].contains(&colour), "{colour:x} is drawn weaker");
    }
    // only colours move: the text, the cells and the cursor are the frame's
    let mut back = faded;
    (back.rows[0][0].fg, back.rows[0][1].fg, back.backgrounds[0].color) = (t.fg, t.ansi[1], t.ansi[4]);
    assert_eq!(back, f);
}

#[test]
fn the_default_is_on_and_keeps_body_text_readable() {
    assert_ne!(theme(&DARK, FADE as f32), DARK, "the fade is on unless the config turns it off");
    for (name, t) in BUILTIN {
        let f = theme(&t, FADE as f32);
        let ratio = contrast(f.fg, f.bg);
        assert!(ratio >= 4.5, "{name}: faded body text is {ratio:.2}:1 on the faded pane");
    }
}

#[test]
fn only_panes_without_the_focus_fade() {
    let of = |shown: &[(&str, bool)], focused, s| -> Vec<f32> {
        targets(shown, focused, s).into_iter().map(|(_, a)| a).collect()
    };
    let two = [("a", false), ("b", false)];
    assert_eq!(of(&two, Some("a"), S), [0., S], "the focused pane is full strength, the other faded");
    assert_eq!(of(&[("a", false)], Some("a"), S), [0.], "one pane alone is not faded");
    assert_eq!(of(&[("a", false), ("b", true), ("c", false)], Some("a"), S), [0., 0., S], "b needs the user");
    assert_eq!(of(&two, Some("a"), 0.), [0., 0.], "the setting is off");
    assert_eq!(of(&two, None, S), [0., 0.], "no focus: nothing fades");
    assert_eq!(of(&two, Some("hidden"), S), [0., 0.], "the focus is on a pane not shown: nothing fades");
    // a focused float or drawer is one of the panes shown: the tiles behind it fade
    assert_eq!(of(&[("a", false), ("b", false), ("float", false)], Some("float"), S), [S, S, 0.]);
    let ids: Vec<String> = targets(&two, Some("a"), S).into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, ["a", "b"]);
}

#[test]
fn a_pane_seen_first_is_at_its_target() {
    let t0 = Instant::now();
    let f = two(t0);
    assert_eq!(amounts(&f, ["a", "b"]), [0., S]);
    assert!(!f.moving(), "opening a tab moves nothing");
    assert_eq!(f.amount("nope"), 0., "a pane it never heard of is full strength");
}

#[test]
fn a_focus_change_fades_over_120_ms_eased_and_then_rests() {
    let t0 = Instant::now();
    let mut f = two(t0);
    let to_b = || targets(&[("a", false), ("b", false)], Some("b"), S);
    f.aim(to_b(), t0, MOTION);
    assert_eq!(amounts(&f, ["a", "b"]), [0., S], "the frame that sees the change draws where it was");
    assert!(f.moving());
    let mut last = [0., S];
    for at in (10..SLIDE.as_millis() as u64).step_by(10) {
        f.aim(to_b(), t0 + ms(at), MOTION);
        let [a, b] = amounts(&f, ["a", "b"]);
        assert!(a > last[0] && a < S, "{at} ms: the pane losing the focus fades out: {a} after {}", last[0]);
        assert!(b < last[1] && b > 0., "{at} ms: the pane gaining it fades in: {b} after {}", last[1]);
        assert!(f.moving(), "{at} ms");
        last = [a, b];
    }
    f.aim(to_b(), t0 + ms(60), MOTION);
    assert!(f.amount("a") > S / 2., "eased out: more than half the way at half the time");
    f.aim(to_b(), t0 + SLIDE, MOTION);
    assert_eq!(amounts(&f, ["a", "b"]), [S, 0.], "done at 120 ms");
    assert!(!f.moving(), "and at rest: no more frames are asked for");
    f.aim(to_b(), t0 + ms(5000), MOTION);
    assert_eq!(amounts(&f, ["a", "b"]), [S, 0.]);
}

#[test]
fn reduce_motion_takes_the_target_at_once() {
    let t0 = Instant::now();
    let mut f = two(t0);
    f.aim(targets(&[("a", false), ("b", false)], Some("b"), S), t0 + ms(1), REDUCED);
    assert_eq!(amounts(&f, ["a", "b"]), [S, 0.], "no frame in between");
    assert!(!f.moving());
}

#[test]
fn a_change_mid_fade_starts_from_where_each_pane_is() {
    let t0 = Instant::now();
    let shown = [("a", false), ("b", false), ("c", false)];
    let mut f = Fades::default();
    f.aim(targets(&shown, Some("a"), S), t0, MOTION);
    f.aim(targets(&shown, Some("b"), S), t0, MOTION);
    f.aim(targets(&shown, Some("b"), S), t0 + ms(40), MOTION);
    let mid = amounts(&f, ["a", "b", "c"]);
    assert!(mid[0] > 0. && mid[0] < S && mid[1] > 0. && mid[1] < S, "a and b are mid-fade: {mid:?}");
    // the focus moves on to c before b arrived
    f.aim(targets(&shown, Some("c"), S), t0 + ms(40), MOTION);
    assert_eq!(amounts(&f, ["a", "b", "c"]), mid, "nothing jumps when the target changes");
    let mut last = mid;
    for at in (50..160).step_by(10) {
        f.aim(targets(&shown, Some("c"), S), t0 + ms(at), MOTION);
        let now = amounts(&f, ["a", "b", "c"]);
        assert!(now[0] >= last[0] && now[1] >= last[1] && now[2] <= last[2], "{at} ms: {now:?} after {last:?}");
        assert!(now.iter().all(|a| (0. ..=S).contains(a)), "{at} ms: {now:?}");
        last = now;
    }
    f.aim(targets(&shown, Some("c"), S), t0 + ms(40) + SLIDE, MOTION);
    assert_eq!(amounts(&f, ["a", "b", "c"]), [S, S, 0.], "exactly one pane at full strength");
    assert!(!f.moving(), "no pane is left half faded");
}

#[test]
fn a_pane_that_left_the_screen_is_forgotten() {
    let t0 = Instant::now();
    let mut f = two(t0);
    f.aim(targets(&[("a", false)], Some("a"), S), t0 + ms(1), MOTION);
    assert!(!f.moving());
    f.aim(targets(&[("a", false), ("b", false)], Some("a"), S), t0 + ms(2), MOTION);
    assert_eq!(amounts(&f, ["a", "b"]), [0., S], "back on screen (another tab, zoom off): in place, no fade in");
    assert!(!f.moving());
}
