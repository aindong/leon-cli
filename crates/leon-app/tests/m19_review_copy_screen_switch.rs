//! Review (M19): copy mode can't outlive the line numbers it points at. A switch to the alternate
//! screen renumbers every line, so copy mode ends (or its cursor is on the new screen).
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;
use serde_json::json;

#[gpui::test]
fn copy_mode_survives_a_screen_switch_on_screen(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let go = s.dir.path().join("go");
    let script = format!(
        "seq 1 300; echo 'alpha beta gamma'; while [ ! -e '{}' ]; do sleep 0.02; done; \
         printf '\\033[?1049h\\033[2J\\033[HALT ONE'; exec cat",
        go.display()
    );
    let id = tab_running(&s, &ws, cx, &script, "alpha beta gamma");
    cx.simulate_keystrokes("cmd-shift-c");
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).is_some(), "⌘⇧C entered copy mode");

    std::fs::write(&go, "").unwrap(); // the program now switches to the alternate screen
    assert!(shows(cx, |cx| view_lines(&ws, cx, &id).iter().any(|l| l == "ALT ONE")), "never switched");

    let Some((at, _)) = ws.read_with(cx, |ws, _| ws.copy_cursor(&id)) else {
        s.app.stop();
        return; // copy mode ended on the switch: acceptable
    };
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let got = s
        .app
        .call(
            "block.method",
            json!({ "session_id": sid, "block_id": id, "method": "dev.leon.terminal.lines",
                    "args": { "from": 0, "count": 1 } }),
        )
        .unwrap();
    let scrolled = got["scrolled"].as_u64().unwrap() as usize;
    let rows = view_lines(&ws, cx, &id).len();
    s.app.stop();
    assert!(
        (scrolled..scrolled + rows).contains(&at.line),
        "copy mode's cursor (line {}) is off the screen now showing (lines {scrolled}..{})",
        at.line,
        scrolled + rows
    );
}
