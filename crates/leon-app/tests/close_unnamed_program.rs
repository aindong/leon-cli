//! Review: a tab whose program the server can't name (another user's: `sudo`, `su`) is still
//! running: closing it asks first, like a tab running `sleep`.
mod close_support;

use close_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_tab_running_a_root_program_asks_first(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    // su is setuid root and waits at its prompt: the pane's foreground leader is root's
    tab_running(&s, &ws, cx, "echo su-ready; exec su -", "Password");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let wid = window(&s.app, &ws, cx).0.id;
    let known = shows(cx, |cx| !ws.read_with(cx, |ws, _| ws.running_programs(&sid, Some(&wid))).is_empty());
    act(&ws, cx, "client.tab.close");
    let asked = cx.has_pending_prompt();
    if asked {
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
    }
    let kept = tabs(&s, &sid) == 2;
    s.app.stop();
    assert!(
        known && asked && kept,
        "a tab running su closed without asking (known={known}, asked={asked}, kept={kept})"
    );
}
