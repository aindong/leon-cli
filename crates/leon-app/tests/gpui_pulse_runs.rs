//! ADR R6 (docs/architecture/gpui.md), phase 3: the Pulse runs only while a shown indicator works,
//! and the poll no longer runs on the spinner's clock.
mod grid_support;

use std::time::Duration;

use gpui::TestAppContext;
use grid_support::*;

const LATER: &str = r"stty -echo; echo ready; read a; printf '\033]7501;state=working\007'; echo working; read b; printf '\033]7501;state=done\007'; echo done; exec sleep 300";

#[gpui::test]
fn the_pulse_runs_only_while_something_works(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, LATER, "ready");
    let pulse = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, cx| ws.pulse_state(cx));
    run_for(cx, Duration::from_millis(500));
    let (running, ticks) = pulse(cx);
    run_for(cx, Duration::from_millis(500));
    assert!(!running && pulse(cx) == (false, ticks), "nothing works: no clock");
    let t = term(&s.app, &id);
    t.write(b"\n").unwrap();
    assert!(shows(cx, |cx| pulse(cx).0), "working: the clock runs");
    let polls = ws.read_with(cx, |ws, _| ws.wakes());
    let ticks = pulse(cx).1;
    run_for(cx, Duration::from_secs(1));
    let (polls, ticks) = (ws.read_with(cx, |ws, _| ws.wakes()) - polls, pulse(cx).1 - ticks);
    assert!(ticks >= 5, "the Pulse ticked {ticks} times in 1 s");
    assert!(polls <= 3, "the wake handler ran {polls} times in 1 s: it keeps no spinner clock");
    t.write(b"\n").unwrap();
    let stopped = shows(cx, |cx| !pulse(cx).0);
    s.app.stop();
    assert!(stopped, "done: the clock stops");
}
