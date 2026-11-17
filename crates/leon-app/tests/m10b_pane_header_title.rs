//! M10b criterion 6, the header part: each pane has a header above its terminal that names its
//! program. Its own binary: its panes run a plain /bin/sh (`plain_shell`), set in the environment
//! before anything starts, which other tests running beside it in one binary must not see.

mod m10b_support;

use gpui::TestAppContext;
use m10b_support::*;

#[gpui::test]
fn each_pane_has_a_header_above_its_terminal(cx: &mut TestAppContext) {
    plain_shell();
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    let (w, _) = window(&s.app, &ws, cx);
    for id in leaves(&w) {
        let header = at(cx, &format!("header-{id}")).expect("a header per pane");
        assert_eq!(f32::from(header.size.height), 24.);
        let term = at(cx, &format!("terminal-{id}")).unwrap();
        assert!(header.bottom() <= term.top(), "above its terminal");
        let title = ws.read_with(cx, |ws, _| ws.pane_title(&id)).expect("a title");
        assert!(title.contains("zsh") || title.contains("sh"), "names the program: {title}");
    }
    s.app.stop();
}
