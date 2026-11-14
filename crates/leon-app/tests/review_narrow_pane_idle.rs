//! Review proof: a pane narrower than two columns settles; redraws don't resize it again and again.

use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;

use gpui::{px, size, TestAppContext};
use leon_app::embedded;
use leon_app::view::{self, Workspace};

#[gpui::test]
fn a_one_column_pane_does_not_resize_on_every_frame(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    cx.update(view::init);
    let a = app.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| Workspace::new(a, window, cx));
    cx.simulate_resize(size(px(240.), px(400.))); // the sidebar leaves a pane a few pixels wide
    cx.run_until_parked();
    let t = ws.read_with(cx, |ws, _| ws.terminal()).unwrap();
    let mut seen = Vec::new();
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        seen.push(t.generation());
    }
    app.stop();
    assert_eq!(seen[1], seen[2], "BUG: every frame resizes the narrow pane: generations {seen:?}");
}
