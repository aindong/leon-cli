//! M9 criterion 5: a split or a new tab starts in the focused pane's working directory.

mod m9_support;

use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn splits_and_tabs_start_in_the_focused_panes_cwd(cx: &mut TestAppContext) {
    let s = setup();
    let dir = s.dir.path().join("work dir");
    std::fs::create_dir(&dir).unwrap();
    let want = real(&dir);
    let (ws, cx) = open(cx, &s.app);
    let first = focused(&s.app, &ws, cx);
    run_until(&first, &format!("cd '{want}' && echo cd-done"), "cd-done");

    cx.simulate_keystrokes("cmd-d");
    let split = focused(&s.app, &ws, cx);
    assert!(!std::sync::Arc::ptr_eq(&first, &split), "the split has the focus");
    run_until(&split, "pwd", &want);

    cx.simulate_keystrokes("cmd-t");
    let tab = focused(&s.app, &ws, cx);
    run_until(&tab, "pwd", &want);
    s.app.stop();
}
